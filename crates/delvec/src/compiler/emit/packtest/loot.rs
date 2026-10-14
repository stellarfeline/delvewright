use super::*;

/// **One loot PackTest per container kind a fill reaches** (spec-0021,
/// spec-0105 §3): the first `loot` fill into each kind, in declaration order.
///
/// A fill is `item replace block … container.<n>`, and what the server takes
/// into a slot depends on the kind standing at the cell — a chest takes
/// anything anywhere, a brewing stand's first three slots take only bottles and
/// potions, a furnace's third is its result — so a fill proven into a chest
/// says nothing about one into a brewing stand. The kind is read off the
/// census of the built world (`containers`: cell → block); a fill whose cell
/// holds no container in this build (owed by a place's detail) has no kind yet
/// and no test. Each test is named for its kind, `v06_loot_<kind>`.
pub(super) fn emit_loot_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    containers: &BTreeMap<[i32; 3], String>,
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    for l in plan.loot.iter().filter(|l| !l.items.is_empty()) {
        let Some(kind) = containers
            .get(&l.cell)
            .and_then(|b| delvewright_dsl::blocks::container_kind(b))
        else {
            continue;
        };
        if !seen.insert(kind.kind) {
            continue;
        }
        let short = kind.kind.trim_start_matches("minecraft:");
        let c = l.cell;
        let mut b = packtest_header(&format!(
            "{title}: loot `{}` fills its {short} on init (spec-0021)",
            l.id
        ));
        b.push(format!("function {ns}:setup"));
        // Slot 0 and the last slot: presence AND identity, so neither a dropped
        // fill nor a shifted slot assignment can pass. Each score is this
        // test's own and is zeroed before it is stored, so a value another
        // template left on a shared holder can never answer for this one.
        let checks = [
            (0usize, &l.items[0]),
            (l.items.len() - 1, l.items.last().unwrap()),
        ];
        for (n, (slot, it)) in checks.iter().enumerate() {
            let holder = format!("#loot_{short}_{n}");
            b.push(format!("scoreboard players set {holder} dw.sys 0"));
            b.push(format!(
                "execute store success score {holder} dw.sys if data block {} {} {} Items[{{Slot:{}b,id:\"{}\"}}]",
                c[0], c[1], c[2], slot, it.item
            ));
            b.push(format!("assert score {holder} dw.sys matches 1"));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_loot_{short}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}
