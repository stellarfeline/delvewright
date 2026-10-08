use super::*;

/// Generated `v08_seal_answers` PackTest: on a live pinned server,
/// a gate that is sealed carries the hitboxes its answer rides, arming is
/// idempotent, and re-opening it takes them away again.
///
/// What this proves and what it deliberately does not: the **presence** contract
/// is fully machine-checkable here, and it is the half that failed — the island's
/// sealed boulder had no hitbox at all, so a press reached nothing. The
/// press-to-actionbar half rides `player_interacted_with_entity`, which no
/// PackTest can fire (it needs a real client's right-click); that primitive is
/// the one every NPC dialogue and bonfire rest already runs on, and the harness
/// bot exercises it there.
///
/// Batch model: the fixture stages the seal itself and hands the world
/// back exactly as it found it — region cleared, hitboxes killed.
pub(super) fn emit_seal_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let Some(s) = plan.seal_hints.first() else {
        return;
    };
    let (from, to) = s.region;
    let n = s.shell_cells().len();
    let tag = format!("dw_seal_{}", s.safe);
    let count = |score: &str| {
        format!(
            "execute store result score #{score} dw.sys if entity @e[type=minecraft:interaction,tag={tag}]"
        )
    };
    let mut b = packtest_header(&format!(
        "{}: the sealed gate `{}` carries an answer the party can press",
        artifact_title(plan.campaign),
        s.anchor
    ));
    b.push(format!("function {ns}:setup"));
    b.push("# Batch model: a sibling test may have driven the campaign past its".to_string());
    b.push("# own seal, so stage a known-OPEN gate rather than assuming one.".to_string());
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!(
        "fill {} {} {} {} {} {} minecraft:air replace {}",
        from[0], from[1], from[2], to[0], to[1], to[2], s.block
    ));
    b.push("# An open gate is nothing to press: nothing armed.".to_string());
    b.push(count("seal_before"));
    b.push("assert score #seal_before dw.sys matches 0".to_string());
    b.push(format!(
        "fill {} {} {} {} {} {} {}",
        from[0], from[1], from[2], to[0], to[1], to[2], s.block
    ));
    b.push(format!(
        "execute unless entity @e[tag={tag}] run function {ns}:{}",
        seal_arm_fn(&s.safe)
    ));
    b.push(count("seal_armed"));
    b.push(format!("assert score #seal_armed dw.sys matches {n}"));
    b.push("# A re-fired seal must not stack a second, co-located set.".to_string());
    b.push(format!(
        "execute unless entity @e[tag={tag}] run function {ns}:{}",
        seal_arm_fn(&s.safe)
    ));
    b.push(count("seal_again"));
    b.push(format!("assert score #seal_again dw.sys matches {n}"));
    b.push("# Re-opening takes the answer down with the stone (no residue).".to_string());
    b.push(format!(
        "fill {} {} {} {} {} {} minecraft:air replace {}",
        from[0], from[1], from[2], to[0], to[1], to[2], s.block
    ));
    b.push(format!("kill @e[tag={tag}]"));
    b.push(count("seal_after"));
    b.push("assert score #seal_after dw.sys matches 0".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v08_seal_answers.mcfunction"),
        lines(&b).into_bytes(),
    );
}
