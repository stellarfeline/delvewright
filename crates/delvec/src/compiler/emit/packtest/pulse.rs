use super::*;

/// The pulse templates (spec-0102 §5.2), synchronous, each ending in
/// `schedule clear` so no chain outlives the template.
///
/// Per gated pulse: `pulse_<s>` drives the gate OPEN, runs the tick's open-edge
/// line, and asserts the latch is set and the chain re-armed itself (one
/// schedule cleared) — a stripped guard on the edge line or a stripped
/// `schedule` reds it; `pulse_<s>_shut` sets the latch, shuts the gate by
/// exactly one term, runs the beat, and asserts the latch cleared and nothing
/// was scheduled — a stripped clearing line or an unguarded `schedule` reds it.
/// Per ungated pulse: `pulse_<s>` runs the beat `setup_finish` starts and
/// asserts it re-armed itself.
///
/// What a PackTest cannot observe is the `playsound` itself: it is a
/// client-bound packet. The command-tree walk proves the line well-formed, and
/// the bot hears it (`harness/src/pulse.ts`).
pub(super) fn emit_pulse_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for p in crate::compiler::pulse::resolve(plan) {
        let s = &p.safe;
        let holder = pulse_holder(s);
        let cleared = format!("#pclr_{s} dw.sys");
        let clear = format!("execute store result score {cleared} run schedule clear {ns}:pulse_{s}");
        let Some(gate) = &p.staged else {
            let mut t = packtest_header(&format!(
                "{title}: pulse `{}` beats and re-arms itself from world load (spec-0102)",
                p.id
            ));
            t.push(format!("scoreboard players set {cleared} 0"));
            t.push(format!("function {ns}:pulse_{s}"));
            t.push(clear.clone());
            t.push(format!("assert score {cleared} matches 1"));
            t.push(format!("scoreboard players reset {cleared}"));
            out.insert(
                format!("packtest-datapack/data/{ns}/test/pulse_{s}.mcfunction"),
                lines(&t).into_bytes(),
            );
            continue;
        };
        // --- the open half ---
        let edge = format!(
            "execute {} unless score {holder} matches 1 run function {ns}:pulse_{s}",
            gate.terms
                .iter()
                .map(|t| t.clause(false))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let mut t = packtest_header(&format!(
            "{title}: pulse `{}` begins beating on the tick its gate opens (spec-0102)",
            p.id
        ));
        t.extend(lethal_gate_lines(gate, true));
        t.push(format!("scoreboard players set {holder} 0"));
        t.push(format!("scoreboard players set {cleared} 0"));
        t.push(edge);
        t.push(format!("assert score {holder} matches 1"));
        t.push(clear.clone());
        t.push(format!("assert score {cleared} matches 1"));
        t.push(format!("scoreboard players set {holder} 0"));
        t.push(format!("scoreboard players reset {cleared}"));
        t.extend(lethal_gate_reset(gate));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/pulse_{s}.mcfunction"),
            lines(&t).into_bytes(),
        );
        // --- the shut half ---
        let mut sh = packtest_header(&format!(
            "{title}: pulse `{}` stops within one interval of its gate shutting (spec-0102)",
            p.id
        ));
        sh.extend(lethal_gate_lines(gate, false));
        sh.push(format!("scoreboard players set {holder} 1"));
        sh.push(format!("scoreboard players set {cleared} 0"));
        sh.push(format!("function {ns}:pulse_{s}"));
        sh.push(format!("assert score {holder} matches 0"));
        sh.push(clear);
        sh.push(format!("assert score {cleared} matches 0"));
        sh.push(format!("scoreboard players set {holder} 0"));
        sh.push(format!("scoreboard players reset {cleared}"));
        sh.extend(lethal_gate_reset(gate));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/pulse_{s}_shut.mcfunction"),
            lines(&sh).into_bytes(),
        );
    }
}

/// The pulse claim (`DW0811`): every declared pulse's `pulse_<s>` body is
/// driven by the suite.
pub(super) fn pulse_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "pulse",
        families: vec!["pulse_".to_string()],
        declared: crate::compiler::pulse::declared_safe(plan),
    }
}
