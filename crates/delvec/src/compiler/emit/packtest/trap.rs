use super::*;

/// v0.6 trap PackTests (spec-0011). A fake player in a 0-player void does not tick
/// entities (the primed-TNT fuse and falling-sand freeze — see the spec's Findings),
/// so a plate → dispenser fire cannot be simulated headlessly; runtime firing
/// coverage is a GameTest concern. What is deterministically checkable in a plain
/// mcfunction — and what these assert — is the compiler's own contract: after
/// `setup`, the trap dispenser holds exactly the declared payload; after the disarm
/// function runs, the payload is gone (the modeled global disarm) and the disarm
/// flag is set. This is the machine-checkable half of acceptance criteria 3 & 4;
/// the plate-fires-and-hits half is the PackTest/GameTest layer the spec records as
/// entity-tick-limited.
pub(super) fn emit_trap_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);

    // Pick the first trap that has both a dispenser payload and a disarm — it
    // exercises both the fill and the empty in one test. Else the first payload trap.
    let dispense_trap = plan
        .traps
        .iter()
        .find(|t| t.dispenser.is_some() && t.payload.is_some());
    let Some(t) = dispense_trap else {
        return;
    };
    let disp = t.dispenser.expect("filtered on Some");
    let (item, count) = t.payload.as_ref().expect("filtered on Some");
    let dis = t.disarm.as_ref();

    let mut b = packtest_header(&format!(
        "{title}: trap `{}` loads its dispenser payload; disarm empties it (spec-0011)",
        t.id
    ));
    b.push(format!("function {ns}:setup"));
    // A 0-player void does not tick entities, so a plate→dispenser fire cannot be
    // simulated here (spec-0011 Findings). Instead place the dispenser and load it
    // with the exact payload the compiler fills, then assert slot 0 is occupied —
    // the machine-checkable "payload lands" contract.
    b.push(format!(
        "setblock {} {} {} minecraft:dispenser",
        disp[0], disp[1], disp[2]
    ));
    b.push(format!(
        "item replace block {} {} {} container.0 with {item} {count}",
        disp[0], disp[1], disp[2]
    ));
    b.push(format!(
        "execute store success score #tload_trap dw.sys if data block {} {} {} Items[0]",
        disp[0], disp[1], disp[2]
    ));
    b.push("assert score #tload_trap dw.sys matches 1".to_string());
    if let Some(dis) = dis {
        // Run the REAL emitted disarm and assert the dispenser is now empty (no ammo
        // → cannot fire) and the disarm flag is set — the trap is provably off.
        // spec-0018: a disarm is a party fact (one lever, everyone's trap off), so
        // the baseline and the assert read `#party`. Cleared first: "never set" is
        // not 0 on the shared-batch server.
        b.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            plan::flag_score(&dis.sets_flag)
        ));
        b.push(format!("function {ns}:trap_disarm_{}", t.safe));
        b.push(format!(
            "execute store success score #tempty_trap dw.sys if data block {} {} {} Items[0]",
            disp[0], disp[1], disp[2]
        ));
        b.push("assert score #tempty_trap dw.sys matches 0".to_string());
        b.push(format!(
            "assert score {} {} matches 1",
            plan::PARTY,
            plan::flag_score(&dis.sets_flag)
        ));
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_trap.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0022 PackTests: the **saturation contract** and the collapse, asserted
/// on a live pinned server.
///
/// The volley test is the runtime half of the saturation contract. It
/// runs the REAL emitted salvo function and then asserts, per standable
/// kill-zone cell, that a projectile exists on the exact trajectory that reaches
/// it — so "the volley blankets its zone" is checked in the game, not just in
/// the compiler. A dummy is parked in one cell, and then MOVED to another
/// between salvos, and each time the occupied cell must show the extra aimed
/// shot on top of the saturation one.
///
/// Assertions are **presence/count based, never "kill everything first"**: this
/// suite runs as one batch on a shared server, so a template that cleared all
/// arrows would sabotage its neighbours. Counting a trajectory that only this
/// volley can produce is residue-robust.
///
/// Boundary worth stating: impact DAMAGE cannot be asserted here. The template
/// harness is synchronous (its only directives are `@dummy` / `@timeout`, with
/// no wait primitive), and an arrow needs ticks of flight to land. What the
/// compiler pins instead is everything damage is a function of — `NoGravity` so
/// the flight path is the proven straight segment, `crit:0b` so the roll is not
/// random, and the exact `Motion` magnitude — leaving the landed-damage check to
/// the bot playthrough (`validation/bot-run.sh`).
pub(super) fn emit_payload_packtests(plan: &Plan, out: &mut BuildOutput, payloads: &PayloadPlans) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);

    if let Some(v) = payloads.volleys.first() {
        let base = format!("volley_{}", v.key);
        let tag = "dw_pt_volley";
        let mut b = packtest_header(&format!(
            "{title}: a volley saturates every standable cell of its kill zone (spec-0022)"
        ));
        b.push(format!("function {ns}:setup"));
        // Pin our own dummy BEFORE any teleport to absolute campaign coords —
        // `@p` would otherwise retarget to a neighbouring test's dummy.
        b.push(format!("tag @p add {tag}"));

        let motion_nbt = |m: [f64; 3]| {
            format!(
                "{{Motion:[{}d,{}d,{}d]}}",
                motion_component(m[0]),
                motion_component(m[1]),
                motion_component(m[2])
            )
        };
        let count_line = |i: usize, m: [f64; 3], holder: &str| {
            format!(
                "execute store result score #{holder}{i} dw.sys if entity \
                 @e[type={},nbt={}]",
                "minecraft:arrow",
                motion_nbt(m)
            )
        };
        // Baselines first: "never set" is not 0 on the shared-batch server.
        for (i, shot) in v.geom.shots.iter().enumerate() {
            b.push(count_line(i, shot.motion, "vbase_"));
        }
        // Park the dummy in the FIRST zone cell, then fire salvo 0.
        let c0 = v.geom.shots[0].cell;
        b.push(format!(
            "tp @a[tag={tag}] {} {} {}",
            f64::from(c0[0]) + 0.5,
            c0[1],
            f64::from(c0[2]) + 0.5
        ));
        b.push(format!("function {ns}:{base}_s0"));
        for (i, shot) in v.geom.shots.iter().enumerate() {
            b.push(count_line(i, shot.motion, "vpost_"));
            // Saturation: EVERY cell gains at least one projectile on its own
            // trajectory, whether or not anyone is standing there. This single
            // family of assertions is the ruling.
            b.push(format!(
                "execute store result score #vgain_{i} dw.sys run scoreboard players get \
                 #vpost_{i} dw.sys"
            ));
            b.push(format!(
                "scoreboard players operation #vgain_{i} dw.sys -= #vbase_{i} dw.sys"
            ));
            let want = if i == 0 { 2 } else { 1 };
            b.push(format!("assert score #vgain_{i} dw.sys matches {want}.."));
        }
        // …and MOVING between salvos does not help: the dummy relocates to a
        // different cell and that cell now takes the extra aimed shot too.
        if v.geom.shots.len() > 1 && v.salvos > 1 {
            let c1 = v.geom.shots[1].cell;
            b.push(format!(
                "tp @a[tag={tag}] {} {} {}",
                f64::from(c1[0]) + 0.5,
                c1[1],
                f64::from(c1[2]) + 0.5
            ));
            for (i, shot) in v.geom.shots.iter().enumerate() {
                b.push(count_line(i, shot.motion, "vmid_"));
            }
            b.push(format!("function {ns}:{base}_s1"));
            for (i, shot) in v.geom.shots.iter().enumerate() {
                b.push(count_line(i, shot.motion, "vend_"));
                b.push(format!(
                    "execute store result score #vg2_{i} dw.sys run scoreboard players get \
                     #vend_{i} dw.sys"
                ));
                b.push(format!(
                    "scoreboard players operation #vg2_{i} dw.sys -= #vmid_{i} dw.sys"
                ));
                let want = if i == 1 { 2 } else { 1 };
                b.push(format!("assert score #vg2_{i} dw.sys matches {want}.."));
            }
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_volley.mcfunction"),
            lines(&b).into_bytes(),
        );
    }

    if let Some(c) = payloads.collapses.first() {
        let base = format!("collapse_{}", c.key);
        let mut b = packtest_header(&format!(
            "{title}: a collapse deletes its region and drops it as falling blocks (spec-0022)"
        ));
        b.push(format!("function {ns}:setup"));
        // Baseline the falling-block population, then bring the roof down.
        b.push(
            "execute store result score #cbase dw.sys if entity @e[type=minecraft:falling_block]"
                .to_string(),
        );
        b.push(format!("function {ns}:{base}"));
        b.push(
            "execute store result score #cpost dw.sys if entity @e[type=minecraft:falling_block]"
                .to_string(),
        );
        b.push("scoreboard players operation #cpost dw.sys -= #cbase dw.sys".to_string());
        b.push(format!(
            "assert score #cpost dw.sys matches {}..",
            c.geom.drops.len()
        ));
        // The region is genuinely gone — this is what the completability proof
        // reasoned about, so it has to be true in the world too.
        for cell in &c.geom.drops {
            b.push(format!(
                "assert block {} {} {} minecraft:air",
                cell[0], cell[1], cell[2]
            ));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_collapse.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// v0.6 trap **gate** PackTest (spec-0011): the gate physically removes and
/// restores the trigger hardware, so the machine-checkable contract is the block
/// itself — while the gate is shut the trigger cell is air (a player stepping there
/// touches nothing), and when it opens the authored trigger is back.
///
/// It drives the gate through the state the campaign really writes — every term on
/// the holder [`Plan::gate_terms`] names, via [`packtest_gate_drive`] — and runs
/// the emitted `trap_gate_tick`, never `trap_gate_on`/`trap_gate_off` directly: a
/// template that calls the two halves itself proves the halves and never the
/// clauses that decide between them, which is how a gate reading a score no
/// player carries shipped behind a green suite.
///
/// Every term is shown able to shut the gate on its own: open it, break exactly
/// that term, tick, assert air; repair it, tick, assert the trigger is back. The
/// first gated trap is the subject, whichever axes it uses.
pub(super) fn emit_trap_gate_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(t) = plan.traps.iter().find(|t| trap_is_gated(t)) else {
        return;
    };
    let gate = trap_gate_of(plan, t);
    let c = t.trigger_cell;
    let party = plan::PARTY;
    let mut b = packtest_header(&format!(
        "{title}: trap `{}` is physically disarmed while its gate is shut, on every term (spec-0011)",
        t.id
    ));
    b.push(format!("function {ns}:setup"));
    // One holder per assert (`#tgate_<n>`), so a red names the step that failed
    // rather than a line number the runner counts its own way.
    let mut step = 0u32;
    let mut run_and_assert = |b: &mut Vec<String>, function: &str, shut: bool| {
        step += 1;
        b.push(format!("function {ns}:{function}"));
        b.push(format!(
            "execute store success score #tgate_{step} dw.sys if block {} {} {} minecraft:air",
            c[0], c[1], c[2]
        ));
        b.push(format!(
            "assert score #tgate_{step} dw.sys matches {}",
            u8::from(shut)
        ));
    };
    // The same truth table twice: once through `trap_gate_init` (what
    // `setup_finish` runs before the first tick) and once through
    // `trap_gate_tick`. The first pass is what sees the world the campaign
    // starts in: a trap whose gate is shut must start disarmed, whichever axis
    // shuts it.
    for function in ["trap_gate_init", "trap_gate_tick"] {
        if function == "trap_gate_tick" {
            // Start shut, so the first open is a transition the tick has to make.
            b.push(format!("function {ns}:trap_gate_off_{}", t.safe));
        }
        b.extend(packtest_gate_drive(plan, gate, true));
        run_and_assert(&mut b, function, false);
        for f in gate.requires_flags {
            let s = plan::flag_score(f.as_str());
            b.push(format!("scoreboard players set {party} {s} 0"));
            run_and_assert(&mut b, function, true);
            b.push(format!("scoreboard players set {party} {s} 1"));
            run_and_assert(&mut b, function, false);
        }
        for f in gate.forbids_flags {
            let s = plan::flag_score(f.as_str());
            b.push(format!("scoreboard players set {party} {s} 1"));
            run_and_assert(&mut b, function, true);
            b.push(format!("scoreboard players set {party} {s} 0"));
            run_and_assert(&mut b, function, false);
        }
        for cmp in gate.requires_state {
            let one = std::slice::from_ref(cmp);
            b.extend(state_drive_lines(plan, one, false));
            run_and_assert(&mut b, function, true);
            b.extend(state_drive_lines(plan, one, true));
            run_and_assert(&mut b, function, false);
        }
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_trap_gate.mcfunction"),
        lines(&b).into_bytes(),
    );
}
