use super::*;

/// spec-0016 §2 shortcut PackTest: the unlock really clears the gate region, and
/// the open is **permanent** — re-running the tick after the sentinel is latched
/// cannot re-seal it, because nothing in the datapack ever fills a shortcut gate
/// (`DW0372` makes that structural at compile time; this asserts the runtime side
/// on a live server). Emits nothing for a campaign with no shortcut.
pub(super) fn emit_shortcut_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(sc) = plan.shortcuts.first() else {
        return;
    };
    let (from, to) = sc.gate_region;
    let probe = from; // one representative cell of the gate region
    let mut b = packtest_header(&format!(
        "{title}: shortcut `{}` opens its gate, permanently (spec-0016 §2)",
        sc.id
    ));
    b.push(format!("function {ns}:setup"));
    // Re-seal the gate and clear the sentinel: a sibling template (or `setup`
    // itself, on a shared batch server) may have left either in any state.
    b.push(format!("scoreboard players set #sc_{} dw.sys 0", sc.safe));
    b.push(format!(
        "fill {} {} {} {} {} {} {}",
        from[0], from[1], from[2], to[0], to[1], to[2], sc.gate_block
    ));
    b.push(format!(
        "execute store success score #sb_scut dw.sys if block {} {} {} {}",
        probe[0], probe[1], probe[2], sc.gate_block
    ));
    b.push("assert score #sb_scut dw.sys matches 1".to_string());
    // Pull the mechanism: the gate is air and the sentinel is latched.
    b.push(format!("function {ns}:shortcut_open_{}", sc.safe));
    b.push(format!(
        "execute store success score #sa_scut dw.sys if block {} {} {} minecraft:air",
        probe[0], probe[1], probe[2]
    ));
    b.push("assert score #sa_scut dw.sys matches 1".to_string());
    b.push(format!("assert score #sc_{} dw.sys matches 1", sc.safe));
    // Permanence: the latched sentinel suppresses any further unlock dispatch, and
    // no emitted function re-fills the region — so a second pass leaves it open.
    b.push(format!("function {ns}:shortcut_open_{}", sc.safe));
    b.push(format!(
        "execute store success score #sp_scut dw.sys if block {} {} {} minecraft:air",
        probe[0], probe[1], probe[2]
    ));
    b.push("assert score #sp_scut dw.sys matches 1".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_shortcut.mcfunction"),
        lines(&b).into_bytes(),
    );
}
