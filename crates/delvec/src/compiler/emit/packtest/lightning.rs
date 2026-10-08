use super::*;

/// spec-0092 PackTests: every declared strike, by its own emitted line, puts a
/// `minecraft:lightning_bolt` at its mark on the tick it runs. The line is the
/// one `emit_lightning` writes into the beat — never a restatement — and the
/// chunk is the one setup holds loaded for it. Emits nothing for a campaign
/// that declares no strike.
pub(super) fn emit_lightning_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for (n, (path, at)) in crate::compiler::lightning::declared(plan)
        .into_iter()
        .enumerate()
    {
        let mut line = Vec::new();
        emit_lightning(plan, at, &mut line);
        let Some(summon) = line.first() else {
            continue; // unresolved mark (`DW0360` owns it)
        };
        let Some(anchor) = anchor_point_any(plan, at.anchor.as_str()) else {
            continue;
        };
        let v = ent_xyz(at.cell(anchor));
        let score = format!("#lb{n}");
        let mut b = packtest_header(&format!(
            "{title}: the lightning at {path} strikes {}",
            at.display()
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("scoreboard players set {score} dw.sys 0"));
        // Setup force-loads the strike's chunk, and a chunk force-loaded on this
        // tick is not loaded yet: a mark off the placed pieces (a bolt out over
        // the water) is a summon into nothing on tick 0. The beat fires long
        // after setup in play; the template waits for the chunk the same way,
        // by a probe it schedules each tick until `execute if loaded` holds.
        let cell = at.cell(anchor);
        let loaded = format!("#lbl{n}");
        let wait = format!("lightning_wait_{n}");
        b.push(format!("scoreboard players set {loaded} dw.sys 0"));
        b.push(format!("function {ns}:{wait}"));
        b.push(format!("await score {loaded} dw.sys matches 1"));
        out.insert(
            format!("packtest-datapack/data/{ns}/function/{wait}.mcfunction"),
            lines(&[
                format!(
                    "execute if loaded {} {} {} run scoreboard players set {loaded} dw.sys 1",
                    cell[0], cell[1], cell[2]
                ),
                format!(
                    "execute unless loaded {} {} {} run schedule function {ns}:{wait} 1t",
                    cell[0], cell[1], cell[2]
                ),
            ])
            .into_bytes(),
        );
        b.push(summon.clone());
        let near = format!(
            "@e[type={},x={},y={},z={},distance=..1]",
            delvewright_dsl::lightning::BOLT_ENTITY,
            v[0],
            v[1],
            v[2]
        );
        b.push(format!(
            "execute if entity {near} run scoreboard players set {score} dw.sys 1"
        ));
        b.push(format!("assert score {score} dw.sys matches 1"));
        b.push(format!("kill {near}"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/lightning_{n}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}
