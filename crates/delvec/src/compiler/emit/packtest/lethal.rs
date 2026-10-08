use super::*;

pub(super) fn emit_lethal_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for v in &plan.lethal_volumes {
        let (lo, hi) = v.region;
        let mid = [
            (lo[0] + hi[0]) / 2,
            (lo[1] + hi[1]) / 2,
            (lo[2] + hi[2]) / 2,
        ];
        let tag = format!("dw_lethtest_{}", v.safe);
        let sel = format!("@e[tag={tag},limit=1]");
        let mut t = packtest_header(&format!(
            "{title}: lethal volume `{}` kills what enters it (spec-0031)",
            v.id
        ));
        t.push(format!("function {ns}:setup"));
        // Never assume a fresh world on the shared-batch server.
        t.push(format!("kill @e[tag={tag}]"));
        t.push(format!(
            "summon minecraft:zombie {} {} {} \
             {{Tags:[\"{tag}\"],NoAI:1b,Silent:1b,PersistenceRequired:1b,Health:20f}}",
            mid[0] as f64 + 0.5,
            mid[1],
            mid[2] as f64 + 0.5
        ));
        // Bound, not assumed: the dummy really is in the volume's own selector
        // box. A template whose dummy landed outside would pass this test by
        // examining nothing, which is the failure mode the ledger exists for.
        t.push(format!(
            "execute store result score #in_leth dw.sys if entity @e[tag={tag},{}]",
            lethal_box(v)
        ));
        t.push("assert score #in_leth dw.sys matches 1".to_string());
        // A staged volume (spec-0088) is driven through its guard with the gate
        // set OPEN, so a stripped sweep reds this template; the `_shut` one
        // below reds a stripped guard.
        match &v.staged {
            Some(gate) => {
                t.extend(lethal_gate_lines(gate, true));
                t.push(format!("function {ns}:lethal_{}_tick", v.safe));
            }
            None => t.push(format!("function {ns}:lethal_{}", v.safe)),
        }
        t.push(format!(
            "execute store result score #hp_leth dw.sys run data get entity {sel} Health 100"
        ));
        t.push("assert score #hp_leth dw.sys matches ..0".to_string());
        t.push(format!("kill @e[tag={tag}]"));
        if let Some(gate) = &v.staged {
            t.extend(lethal_gate_reset(gate));
            // --- the shut half: the guard withholds the volume completely ---
            let stag = format!("dw_lethshut_{}", v.safe);
            let ssel = format!("@e[tag={stag},limit=1]");
            let mut sh = packtest_header(&format!(
                "{title}: staged lethal volume `{}` withholds its kill while its gate is shut \
                 (spec-0088)",
                v.id
            ));
            sh.push(format!("function {ns}:setup"));
            sh.push(format!("kill @e[tag={stag}]"));
            sh.push(format!(
                "summon minecraft:zombie {} {} {} \
                 {{Tags:[\"{stag}\"],NoAI:1b,Silent:1b,PersistenceRequired:1b,Health:20f}}",
                mid[0] as f64 + 0.5,
                mid[1],
                mid[2] as f64 + 0.5
            ));
            sh.push(format!(
                "execute store result score #in_lshut dw.sys if entity @e[tag={stag},{}]",
                lethal_box(v)
            ));
            sh.push("assert score #in_lshut dw.sys matches 1".to_string());
            sh.extend(lethal_gate_lines(gate, false));
            sh.push(format!("function {ns}:lethal_{}_tick", v.safe));
            sh.push(format!(
                "execute store result score #hp_lshut dw.sys run data get entity {ssel} Health"
            ));
            sh.push("assert score #hp_lshut dw.sys matches 20".to_string());
            sh.push(format!("kill @e[tag={stag}]"));
            sh.extend(lethal_gate_reset(gate));
            out.insert(
                format!(
                    "packtest-datapack/data/{ns}/test/lethal_{}_shut.mcfunction",
                    v.safe
                ),
                lines(&sh).into_bytes(),
            );
        }
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/lethal_{}.mcfunction",
                v.safe
            ),
            lines(&t).into_bytes(),
        );

        // --- the wording guard, on a real player object ---
        //
        // The template above proves the volume KILLS. This one proves it does not
        // CLAIM a death it did not cause — the half that was wrong twice.
        //
        // **What this tier can and cannot witness, measured rather than assumed.**
        // A PackTest fake player is **permanently undamageable**: on the pinned
        // 1.21.11 toolserver a dummy in `playerGameType: 0` with `Invulnerable: 0`
        // and `Health: 20f` stood inside a volume whose loop swings every tick and
        // was still at `Health: 20f` after **202 ticks** — far past vanilla's
        // 59-tick post-respawn window — and `minecraft:generic` was refused
        // identically, so it is not damage-type specific. `/damage` reported
        // **success** throughout, which is why the guard cannot be
        // `execute store success` and reads the outcome instead ([`LETHAL_HP`]).
        //
        // So a player DEATH cannot be witnessed here at all, and this template
        // does not pretend to: that claim belongs to the bot tier, which drives a
        // real client. What the dummy is perfect for is the other direction — it
        // is a body that provably never dies, which makes it a standing fixture
        // for "nothing landed", stronger than the 3-second respawn window because
        // it never expires. An unconditional `tellraw` (the first version of this
        // verb) would print the volume's wording here every tick, forever, about a
        // death that is not happening.
        let (pin, psel) = pin_dummy(&format!("dw_lethp_{}", v.safe));
        let mut p = packtest_header(&format!(
            "{title}: lethal volume `{}` withholds its wording when the blow does not land \
             (spec-0031)",
            v.id
        ));
        p.push(format!("function {ns}:setup"));
        p.push(pin);
        p.push(format!(
            "tp {psel} {} {} {}",
            mid[0] as f64 + 0.5,
            mid[1],
            mid[2] as f64 + 0.5
        ));
        // Baseline the guard to the DEAD sentinel, so a kill function that never
        // ran at all cannot pass this by leaving the score untouched — the
        // binding, not the outcome, is what a zero would hide.
        p.push(format!("scoreboard players set {LETHAL_HP} 0"));
        // The volume's own DRIVER, not its kill function: the driver is what
        // carries the `@a[<box>]` re-bind, so this binds to the player path
        // existing and reaching a player standing in the box. Calling
        // `lethal_<id>_kill` directly — as the first version did — passes
        // unchanged with the player line deleted from the driver, which is a test
        // that examines the wrong object and reports green.
        p.push(format!("function {ns}:lethal_{}", v.safe));
        p.push(format!("assert score {LETHAL_HP} matches 1.."));
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/lethal_{}_claim.mcfunction",
                v.safe
            ),
            lines(&p).into_bytes(),
        );
    }
}
