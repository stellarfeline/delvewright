//! Runtime state and flags (spec-0031): declarations, conditions, drives, named states.

use super::*;

/// The declaration of a runtime datum, or `None` if the campaign declares none
/// by that id (which validation has already rejected — `DW0500`).
pub(super) fn state_decl<'a>(
    plan: &'a Plan,
    id: &StateId,
) -> Option<&'a delvewright_dsl::StateDecl> {
    plan.campaign.quests.content.state_decl(id.as_str())
}

/// **Who** holds a datum's value: the party fake player for a `party` datum, the
/// acting player for a `player` one.
///
/// The whole content of the declared scope, in one function. An undeclared datum
/// answers `#party` so that a campaign which failed validation still emits
/// something well-formed rather than panicking mid-build.
pub(super) fn state_holder(plan: &Plan, id: &StateId) -> String {
    match state_decl(plan, id).map(|s| s.scope) {
        Some(StateScope::Player) => "@s".to_string(),
        _ => plan::PARTY.to_string(),
    }
}

/// A datum's declared `initial` — the value it starts at and the value
/// `clear-state` returns it to.
pub(super) fn state_initial(plan: &Plan, id: &StateId) -> i32 {
    state_decl(plan, id).map(|s| s.initial).unwrap_or(0)
}

/// The `execute` sub-clauses a gate's **numeric** terms contribute, each already
/// prefixed with a space, e.g. ` if score #party dw.s_purse matches 500..`.
///
/// With `negate` the clauses assert the gate is NOT satisfied — one clause per
/// term, which is the "any single term failing shuts it" form the trap arming
/// tick needs (there, a gate closing has to be expressible as its own condition).
///
/// Empty for a gate with no comparison, which is every pre-0.10 campaign — so
/// splicing this into an existing guard moves no existing command by a byte.
pub(super) fn state_clauses(plan: &Plan, cmps: &[StateCompare], negate: bool) -> Vec<String> {
    // `Plan::state_terms` decides the holder, the range and which keyword the
    // comparison wants; `negate` flips that keyword and nothing else, so the two
    // readings can never disagree about what the range means.
    plan.state_terms(cmps)
        .iter()
        .map(|t| t.clause(negate))
        .collect()
}

/// [`state_clauses`] in the **space-prefixed** form the `execute` builders in
/// this module splice onto a growing condition (` if score … if score …`).
pub(super) fn state_cond(plan: &Plan, cmps: &[StateCompare], negate: bool) -> String {
    state_clauses(plan, cmps, negate)
        .into_iter()
        .map(|c| format!(" {c}"))
        .collect()
}

/// The whole gate as one space-prefixed `execute` condition — flags, then the
/// negative flags, then the numeric terms, in gate field order.
///
/// Empty for an ungated site, so a caller that splices it in unconditionally
/// emits exactly what it emitted before v0.10.
pub(super) fn gate_cond(plan: &Plan, gate: Gate<'_>) -> String {
    plan.gate_terms(gate)
        .iter()
        .map(|t| format!(" {}", t.clause(false)))
        .collect()
}

/// The commands that force a gate's numeric terms to be satisfied (`satisfy`) or
/// violated — used by the generated PackTest preambles, which have to *drive* a
/// gate rather than merely read it.
///
/// A `not-equals` gate is satisfied by any other value, and `value + 1` is the
/// deterministic choice; `at-least`/`at-most` are satisfied at the boundary. The
/// violating value is the mirror. Both directions are needed because the flag
/// gate's own templates already prove both truth-table rows, and a numeric gate
/// that only ever proved the open row would be the weaker test.
pub(super) fn state_drive_lines(plan: &Plan, cmps: &[StateCompare], satisfy: bool) -> Vec<String> {
    cmps.iter()
        .map(|c| {
            format!(
                "scoreboard players set {} {} {}",
                state_holder(plan, &c.state),
                plan::state_score(c.state.as_str()),
                state_drive_value(c, satisfy)
            )
        })
        .collect()
}

/// **Every numeric term of one display condition, satisfied together.**
///
/// [`state_drive_value`] answers for one term; a condition can hold several on
/// one datum (an option's own `at-most 5` beside its objective's `at-most 9`),
/// and driving each to its own boundary leaves the datum at whichever was
/// written last. Per datum, the value is the first candidate — each term's own
/// satisfying value, then each one's boundary neighbourhood, in term order —
/// that meets every term on it; a break of one term is the first candidate that
/// violates it and meets the rest, so the negative assert isolates that term.
/// With one term per datum both are exactly [`state_drive_value`]'s. A datum
/// no candidate satisfies (a condition that can never hold) keeps the per-term
/// drive, so the PackTest says the option is never shown.
pub(super) struct JointStateDrive<'a> {
    /// Datum id → every term the condition reads it with, in term order.
    terms: BTreeMap<&'a str, Vec<&'a StateCompare>>,
    /// Datum id → the one value meeting all of its terms, where one exists.
    value: BTreeMap<&'a str, i32>,
}

impl<'a> JointStateDrive<'a> {
    pub(super) fn of(cmps: &[&'a StateCompare]) -> Self {
        let mut terms: BTreeMap<&str, Vec<&StateCompare>> = BTreeMap::new();
        for c in cmps {
            terms.entry(c.state.as_str()).or_default().push(c);
        }
        let value = terms
            .iter()
            .filter_map(|(datum, on)| {
                let v = on
                    .iter()
                    .map(|c| state_drive_value(c, true))
                    .chain(on.iter().flat_map(|c| state_candidates(c)))
                    .find(|v| on.iter().all(|c| c.holds(*v)))?;
                Some((*datum, v))
            })
            .collect();
        JointStateDrive { terms, value }
    }

    /// The satisfying value of `c`'s datum: the joint one, else `c`'s own.
    fn satisfy(&self, c: &StateCompare) -> i32 {
        self.value
            .get(c.state.as_str())
            .copied()
            .unwrap_or_else(|| state_drive_value(c, true))
    }

    /// A value that breaks `c` and, where one exists, holds every other term on
    /// its datum. Terms are compared by value, so an identical twin of `c` is
    /// broken with it — it is the same condition.
    fn break_one(&self, c: &StateCompare) -> i32 {
        let own = state_drive_value(c, false);
        let on: &[&StateCompare] = self
            .terms
            .get(c.state.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let others: Vec<&&StateCompare> = on.iter().filter(|o| ***o != *c).collect();
        std::iter::once(own)
            .chain(on.iter().flat_map(|o| state_candidates(o)))
            .find(|v| !c.holds(*v) && others.iter().all(|o| o.holds(*v)))
            .unwrap_or(own)
    }

    /// The satisfying line of each of `cmps`, and each one's break/restore pair.
    pub(super) fn drive(
        &self,
        plan: &Plan,
        cmps: &[StateCompare],
        satisfied: &mut Vec<String>,
        terms: &mut Vec<(Vec<String>, Vec<String>)>,
    ) {
        let line = |c: &StateCompare, v: i32| {
            format!(
                "scoreboard players set {} {} {v}",
                state_holder(plan, &c.state),
                plan::state_score(c.state.as_str()),
            )
        };
        for c in cmps {
            satisfied.push(line(c, self.satisfy(c)));
        }
        for c in cmps {
            terms.push((
                vec![line(c, self.break_one(c))],
                vec![line(c, self.satisfy(c))],
            ));
        }
    }
}

/// The values a term's boundary separates: its value and either neighbour.
pub(super) fn state_candidates(c: &StateCompare) -> [i32; 3] {
    [c.value, c.value.wrapping_add(1), c.value.wrapping_sub(1)]
}

/// The one deterministic value that satisfies (or violates) a single numeric
/// term — [`state_drive_lines`]' value column, exposed on its own because the
/// cast-ladder proof needs the number (to evaluate the ladder model at it)
/// and not just the line. One table, so the drive and the model can never
/// disagree about what value a broken term was broken to.
pub(super) fn state_drive_value(c: &StateCompare, satisfy: bool) -> i32 {
    match (c.op, satisfy) {
        // The boundary satisfies `equals`, `at-least` and `at-most`.
        (CompareOp::Equals, true)
        | (CompareOp::AtLeast, true)
        | (CompareOp::AtMost, true)
        | (CompareOp::NotEquals, false) => c.value,
        // One step past it violates them — and satisfies `not-equals`.
        (CompareOp::Equals, false) | (CompareOp::AtMost, false) | (CompareOp::NotEquals, true) => {
            c.value.wrapping_add(1)
        }
        (CompareOp::AtLeast, false) => c.value.wrapping_sub(1),
    }
}

/// Every declared runtime datum, in declared order (empty for a pre-0.10
/// campaign, which is what keeps its setup byte-identical).
pub(super) fn declared_states(c: &delvewright_dsl::Campaign) -> &[delvewright_dsl::StateDecl] {
    &c.quests.content.state
}

/// The shadow objective holding the value a **named** datum was last announced at
/// (DSL v0.10, spec-0032).
pub(super) fn state_shadow_score(id: &str) -> String {
    format!("dw.sh_{}", plan::safe_local(id))
}

/// Every declared datum that carries a player-visible `name` — i.e. every currency.
/// Empty for a campaign that names none, which is what keeps a spec-0031 campaign
/// byte-identical.
pub(super) fn named_states<'a>(plan: &'a Plan) -> Vec<&'a delvewright_dsl::StateDecl> {
    declared_states(plan.campaign)
        .iter()
        .filter(|st| st.name.is_some())
        .collect()
}

/// Every declared datum that **stands** on screen (spec-0076), paired with the
/// slot it declares. At most one, by `DW0919`; empty for a campaign that declares
/// none, which is what keeps a spec-0032 campaign byte-identical.
pub(super) fn standing_states<'a>(
    plan: &'a Plan,
) -> Vec<(
    &'a delvewright_dsl::StateDecl,
    delvewright_dsl::StateDisplay,
)> {
    declared_states(plan.campaign)
        .iter()
        .filter_map(|st| st.display.map(|d| (st, d)))
        .collect()
}

/// **A named datum announces itself when it changes** (DSL v0.10, spec-0032): the
/// tick driver and the per-datum announcer behind it.
///
/// The announcement belongs to the DATUM, not to each verb that writes it — which
/// is the same rule that put the numeric comparison in the gate rather than in the
/// shop. Hanging it off the write sites looked simpler and was wrong twice over:
/// there are five of them (three state verbs, the stake's forfeit, the stake's
/// restore) so the readout would be five copies, and — the defect this shape was
/// written to fix, found by reading the generated `shop_pick_0_0` — a readout
/// emitted inside a gated effect carries that effect's gate, which is evaluated
/// AFTER the write it reports. Spend your last ember behind a
/// `requires_state: at-least 1` gate and the balance moves to 0, so the readout's
/// inherited guard no longer holds and the one change the player most needs to see
/// is the one they are never told about.
///
/// A shadow score per named datum removes the whole class: nothing consults a gate,
/// the announcement fires on **any** change from any cause — a purchase, a death's
/// forfeit, a stake collected, a plain `set-state` — and it fires exactly once per
/// change. The shadow is seeded alongside the datum itself, so joining a world
/// announces nothing.
pub(super) fn named_state_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for st in named_states(plan) {
        let id = st.id.as_str();
        let (obj, shadow) = (plan::state_score(id), state_shadow_score(id));
        let f = format!("st_show_{}", plan::safe_local(id));
        match st.scope {
            StateScope::Player => out.push(format!(
                "execute as @a unless score @s {obj} = @s {shadow} run function {ns}:{f}"
            )),
            StateScope::Party => out.push(format!(
                "execute unless score {p} {obj} = {p} {shadow} run function {ns}:{f}",
                p = plan::PARTY
            )),
        }
    }
    out
}

/// The `st_show_<datum>` functions: say the balance, then remember having said it.
///
/// The value travels as vanilla's own `{"score":…}` component, so the line the
/// player reads is the live balance rather than a number baked at emit time, and
/// the name travels as a `{translate, fallback}` component like every other
/// authored string.
pub(super) fn emit_named_state_functions(plan: &Plan) -> Vec<(String, String)> {
    let mut fns = Vec::new();
    for st in named_states(plan) {
        let id = st.id.as_str();
        let (obj, shadow) = (plan::state_score(id), state_shadow_score(id));
        let name = st.name.as_deref().unwrap_or_default();
        let (who, holder) = match st.scope {
            StateScope::Player => ("@s".to_string(), "@s".to_string()),
            StateScope::Party => ("@a".to_string(), plan::PARTY.to_string()),
        };
        let component = json!([
            { "text": "" },
            tr(name),
            { "text": ": " },
            { "score": { "name": holder, "objective": obj }, "color": "gold" }
        ]);
        fns.push((
            format!("st_show_{}", plan::safe_local(id)),
            lines(&[
                format!("title {who} actionbar {component}"),
                format!("scoreboard players operation {holder} {shadow} = {holder} {obj}"),
            ]),
        ));
    }
    fns
}

/// The party flag gate for a list of flags: an ` if score #party dw.f_<flag>
/// matches 1` fragment per flag (leading space), or `""` for an ungated list.
///
/// spec-0018 replaced the pre-party spelling — an `@a[scores={dw.f_a=1..}]`
/// selector asking "does some player hold it" — with a single party read. The
/// selector form is now not merely redundant but *wrong*: nothing writes a flag
/// onto a player any more, so it would never match.
pub(super) fn party_flag_gate(flags: &[delvewright_dsl::FlagId]) -> String {
    flags
        .iter()
        .map(|f| {
            format!(
                " if score {} {} matches 1",
                plan::PARTY,
                plan::flag_score(f.as_str())
            )
        })
        .collect()
}

/// The flags any `set-flag` effect produces (sorted, deduped) — quest effects,
/// plus (DSL v0.4) dialogue `set-flag` effects and environment-trigger effects.
/// Empty extra sources for v0.2/v0.3, keeping their scoreboard setup identical.
///
/// **This is emission, not a lint**. A `set-flag` whose `dw.f_<flag>`
/// objective is missing from `setup` writes to nothing: vanilla answers
/// `scoreboard players set … <undeclared> 1` with a command error and carries on,
/// so there is no crash, nothing a bot observes, and every gate on that flag
/// simply never opens. It is the `DW0497` shape — a call with no callee —
/// reproduced one layer down, at the scoreboard.
///
/// The roots therefore come from [`crate::compiler::plan::for_each_effect_root`], the one
/// enumeration [`all_campaign_effects`] itself walks, so the declaration walk and
/// the write walk cannot disagree about where a `set-flag` may live. This
/// inventory used to hand-list three of the five, and a `set-flag` in a
/// `traps[].payload` or a dialogue option's `set-checkpoint` `on_respawn` bundle
/// emitted its write against an objective nothing had created.
///
/// Depth was never the blind spot — `visit_deep` already descended `sequence`
/// steps and lifecycle bundles — so the fix is which lists the descent starts
/// from, and it is inherited rather than re-listed.
///
/// The sources below the walk are the ones that are **not** effect roots and so
/// cannot come from it: a trap's and a timed gate's `disarm.sets_flag`, the flat
/// `DialogueEffect::SetFlag` list (a `DialogueEffect` is not a `QuestEffect`), and
/// the cast ledger's flag *reads*.
pub(super) fn declared_flags(c: &delvewright_dsl::Campaign) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    crate::compiler::plan::for_each_effect_root(c, &mut |_site, effs| {
        for eff in effs {
            eff.visit_deep(&mut |e| {
                if let Some(f) = e.set_flag() {
                    out.insert(f.as_str().to_string());
                }
            });
        }
    });
    // v0.6 traps (spec-0011): a disarm's `sets_flag` needs its own scoreboard.
    for t in &c.quests.content.traps {
        if let Some(dis) = &t.disarm {
            out.insert(dis.sets_flag.as_str().to_string());
        }
    }
    // A timed gate's disarm sets a flag exactly as a trap's does.
    for g in &c.quests.content.timed_gates {
        if let Some(dis) = &g.disarm {
            out.insert(dis.sets_flag.as_str().to_string());
        }
    }
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for opt in &node.options {
                for eff in &opt.effects {
                    if let Some(f) = eff.set_flag() {
                        out.insert(f.as_str().to_string());
                    }
                }
            }
        }
    }
    // v0.7 cast ledger (spec-0020): a per-branch cast READS its branch flags in
    // the scene selector. Reading an objective that was never declared is a
    // runtime command error, and unlike a `set-flag` write there is nothing
    // elsewhere that guarantees the declaration — a branch may legitimately be
    // gated on a flag some *other* campaign path sets. Declared here so the read
    // is always well-formed.
    for q in &c.quests.content.quests {
        for entry in q.cast.values() {
            for p in entry.placements() {
                for f in p.requires_flags.iter().chain(&p.forbids_flags) {
                    out.insert(f.as_str().to_string());
                }
            }
        }
    }
    out
}
