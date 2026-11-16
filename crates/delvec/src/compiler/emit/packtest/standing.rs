use super::*;

/// The standing line of a `party` datum (spec-0076), synchronous, per standing
/// `party` datum: `state_<d>_stands` runs `setup` and asserts the visible line
/// (`dw.party`) holds the party value; moves the party value and asserts the
/// line has not moved yet; runs the datum's own tick driver — the line `tick`
/// carries — and asserts the line now holds the moved value; then puts the
/// value back. A line seeded wrong, a line the announcer never copies to, or a
/// copy aimed at another holder or objective reds it.
///
/// Synchronous on purpose: every sibling's `setup` re-seeds `#party`, and a
/// body that straddled a tick could read a sibling's seed instead of its own
/// change. The tick driver is the same function `tick` emits its line from.
pub(super) fn emit_standing_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for (st, display) in super::super::standing_states(plan) {
        if super::super::party_line_copy(st).is_none() {
            continue;
        }
        let id = st.id.as_str();
        let obj = plan::state_score(id);
        let (line, party) = (plan::PARTY_LINE, plan::PARTY);
        let same = format!(
            "execute store success score #sd_same dw.sys if score {line} {obj} = {party} {obj}"
        );
        let mut t = packtest_header(&format!(
            "{title}: party datum `{id}` stands on the {} as the party's one line, and the line \
             follows a party change (spec-0076)",
            display.slot()
        ));
        t.push(format!("function {ns}:setup"));
        t.push(same.clone());
        t.push("assert score #sd_same dw.sys matches 1".to_string());
        t.push(format!("scoreboard players add {party} {obj} 7"));
        t.push(same.clone());
        t.push("assert score #sd_same dw.sys matches 0".to_string());
        t.push(super::super::named_state_driver(plan, st));
        t.push(same.clone());
        t.push("assert score #sd_same dw.sys matches 1".to_string());
        t.push(format!("scoreboard players remove {party} {obj} 7"));
        t.push(super::super::named_state_driver(plan, st));
        t.push(same);
        t.push("assert score #sd_same dw.sys matches 1".to_string());
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/state_{}_stands.mcfunction",
                plan::safe_local(id)
            ),
            lines(&t).into_bytes(),
        );
    }
}
