//! Pulses (spec-0102 §4.2).

use super::*;

/// The latch holder of a pulse: `#pulse_<s>` on `dw.sys`.
pub(super) fn pulse_holder(safe: &str) -> String {
    format!("#pulse_{safe} dw.sys")
}

/// The gate's terms as one `execute` sub-clause run — `<term> <term> …` — each
/// written by [`plan::GateTerm::clause`], the formatter every guard is written
/// by.
fn pulse_terms(gate: &plan::StagedGate) -> String {
    gate.terms
        .iter()
        .map(|t| t.clause(false))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `setup_finish` lines for the campaign's pulses, in declaration order: a
/// gated pulse's latch seeded to 0, an ungated pulse's first beat. Empty for a
/// campaign that declares none, so its `setup_finish` is byte-identical.
pub(super) fn pulse_setup(plan: &Plan, rows: &[crate::compiler::pulse::PulseRow]) -> Vec<String> {
    let ns = &plan.namespace;
    rows.iter()
        .map(|r| match &r.plan.staged {
            Some(_) => format!("scoreboard players set {} 0", pulse_holder(&r.plan.safe)),
            None => format!("function {ns}:pulse_{}", r.plan.safe),
        })
        .collect()
}

/// The tick's open-edge line per gated pulse (spec-0102 §4.2): the gate holds
/// and the latch is not set, so the first beat sounds on the tick the gate
/// opens and the chain carries itself from there. Empty for a campaign whose
/// pulses are all ungated.
pub(super) fn pulse_tick(plan: &Plan, rows: &[crate::compiler::pulse::PulseRow]) -> Vec<String> {
    let ns = &plan.namespace;
    rows.iter()
        .filter_map(|r| {
            let gate = r.plan.staged.as_ref()?;
            Some(format!(
                "execute {} unless score {} matches 1 run function {ns}:pulse_{}",
                pulse_terms(gate),
                pulse_holder(&r.plan.safe),
                r.plan.safe
            ))
        })
        .collect()
}

/// The one `playsound` a beat speaks: every player standing in the box, but
/// a player watching a cutscene, at the source cell's centre, at the derived
/// volume and the declared pitch, on `master` as `play-sound` writes it.
pub(super) fn pulse_playsound(r: &crate::compiler::pulse::PulseRow) -> String {
    let p = &r.plan;
    let (lo, hi) = p.heard;
    let at = p.point();
    format!(
        "playsound {} master @a[{},tag=!{CUTSCENE_TAG}] {} {} {} {} {}",
        p.sound,
        box_selector_args(lo, hi),
        fmt_f64(at[0]),
        fmt_f64(at[1]),
        fmt_f64(at[2]),
        fmt_f64(r.volume),
        fmt_f64(p.pitch)
    )
}

/// Generate one function per pulse (spec-0102 §4.2): `pulse_<s>`.
///
/// Gated: latch on, the beat and the next hop under the gate, then one
/// clearing line per term — *not (every term holds)* is a disjunction, the
/// trap gate's shape — so the chain dies within one interval of the gate
/// shutting and the tick line re-arms it on the next opening. Ungated: the
/// beat and the next hop alone. `schedule … replace` makes a second chain
/// impossible by construction.
pub(super) fn emit_pulse_functions(
    plan: &Plan,
    rows: &[crate::compiler::pulse::PulseRow],
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for r in rows {
        let p = &r.plan;
        let beat = pulse_playsound(r);
        let next = format!(
            "schedule function {ns}:pulse_{} {}t replace",
            p.safe, p.every
        );
        let body = match &p.staged {
            None => vec![beat, next],
            Some(gate) => {
                let terms = pulse_terms(gate);
                let holder = pulse_holder(&p.safe);
                let mut b = vec![
                    format!("scoreboard players set {holder} 1"),
                    format!("execute {terms} run {beat}"),
                    format!("execute {terms} run {next}"),
                ];
                b.extend(gate.terms.iter().map(|t| {
                    format!(
                        "execute {} run scoreboard players set {holder} 0",
                        t.clause(true)
                    )
                }));
                b
            }
        };
        fns.push((format!("pulse_{}", p.safe), lines(&body)));
    }
    fns
}
