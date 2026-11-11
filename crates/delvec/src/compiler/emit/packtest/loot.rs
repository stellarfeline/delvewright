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
        let b = loot_template(ns, &title, l, short);
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_loot_{short}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// One loot template's lines: the fill, then what the container holds.
///
/// A fill that runs at load is read after `setup`. A fill that waits for its
/// beat (`filled: "by-effect"`, spec-0134 §6) is run by the template itself —
/// `function <ns>:fill_<loot>`, the function `fill-loot` calls — because
/// nothing at load fills it: a template that only read the container passed
/// or failed by whichever other template had completed the beat in the shared
/// world before it.
fn loot_template(
    ns: &str,
    title: &str,
    l: &crate::compiler::plan::LootPlan,
    short: &str,
) -> Vec<String> {
    {
        let c = l.cell;
        let mut b = packtest_header(&if l.by_effect {
            format!(
                "{title}: loot `{}` fills its {short} when its beat runs it (spec-0134)",
                l.id
            )
        } else {
            format!(
                "{title}: loot `{}` fills its {short} on init (spec-0021)",
                l.id
            )
        });
        b.push(format!("function {ns}:setup"));
        if l.by_effect {
            b.push(format!(
                "function {ns}:{}",
                super::super::loot::fill_fn(&l.id)
            ));
        }
        // Slot 0 and the last slot: presence AND identity, so neither a dropped
        // fill nor a shifted slot assignment can pass. Each score is this
        // test's own and is zeroed before it is stored, so a value another
        // template left on a shared holder can never answer for this one.
        // A named stack's slot is its own (spec-0111 §3.3); an unnamed one's
        // is its position.
        let slot_of = |n: usize, it: &crate::compiler::plan::LootItemPlan| {
            it.slot
                .as_deref()
                .and_then(delvewright_dsl::holders::container_index)
                .unwrap_or(n)
        };
        let last = l.items.len() - 1;
        let checks = [
            (slot_of(0, &l.items[0]), &l.items[0]),
            (slot_of(last, &l.items[last]), &l.items[last]),
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
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(by_effect: bool) -> crate::compiler::plan::LootPlan {
        let item = |i: &str| crate::compiler::plan::LootItemPlan {
            stack: delvewright_dsl::Stack::of(i),
            slot: None,
        };
        crate::compiler::plan::LootPlan {
            id: "loot/stores".to_string(),
            anchor: "anchor/barrel".to_string(),
            cell: [2, 67, 29],
            items: vec![item("minecraft:bread"), item("minecraft:potion")],
            by_effect,
        }
    }

    /// A fill that waits for its beat is run by its own template, between
    /// `setup` and the first read; a fill at load is only read. Without the
    /// call the by-effect template reads a container nothing has filled.
    #[test]
    fn a_fill_that_waits_is_run_by_its_own_template() {
        let at = |b: &[String], what: &str| b.iter().position(|l| l.contains(what));
        let waits = loot_template("ns", "T", &fill(true), "barrel");
        let (setup, run, read) = (
            at(&waits, "function ns:setup").expect("setup"),
            at(&waits, "function ns:fill_stores").expect("the fill's own function"),
            at(&waits, "execute store success").expect("a read"),
        );
        assert!(setup < run && run < read, "{waits:#?}");
        assert!(
            waits
                .iter()
                .any(|l| l.contains("Slot:1b,id:\"minecraft:potion\""))
        );
        let loads = loot_template("ns", "T", &fill(false), "barrel");
        assert!(at(&loads, "fill_stores").is_none(), "{loads:#?}");
        assert!(at(&loads, "on init").is_some());
    }
}
