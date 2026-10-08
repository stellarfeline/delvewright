use super::*;

pub(super) fn emit_loot_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(l) = plan.loot.iter().find(|l| !l.items.is_empty()) else {
        return;
    };
    let c = l.cell;
    let mut b = packtest_header(&format!(
        "{title}: loot `{}` fills its container on init (spec-0021)",
        l.id
    ));
    b.push(format!("function {ns}:setup"));
    // Slot 0 and the last slot: presence AND identity, so neither a dropped
    // fill nor a shifted slot assignment can pass.
    let checks = [
        (0usize, &l.items[0]),
        (l.items.len() - 1, l.items.last().unwrap()),
    ];
    for (n, (slot, it)) in checks.iter().enumerate() {
        b.push(format!(
            "execute store success score #loot{n} dw.sys if data block {} {} {} Items[{{Slot:{}b,id:\"{}\"}}]",
            c[0], c[1], c[2], slot, it.item
        ));
        b.push(format!("assert score #loot{n} dw.sys matches 1"));
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_loot.mcfunction"),
        lines(&b).into_bytes(),
    );
}
