use super::*;

/// **The assembly's generated PackTests** (spec-0082 §10), per placed
/// assembly `<s>`:
///
/// * `asm_spawn_<s>` — the real `asm_spawn_<s>` stands one root with every
///   rig part riding it (counted off the root's passengers, one
///   `scoreboard players add` each — a forked `store result` counts one
///   branch, spec-0082 §8 row 2) and one hitbox when declared; the real
///   `asm_despawn_<s>` leaves no entity of the assembly behind.
/// * `asm_hits_<s>_<trigger>` — per `strike-assembly` trigger whose bundle
///   counts a `party` datum with an ungated `add-state`: an `attack` record
///   written onto the hitbox and the real `tick` move the datum by the amount;
///   and where a `play-clip` in the bundle waits on that datum at a count, the
///   blow that reaches the count makes that clip the one the assembly plays.
/// * `asm_land_<s>` — per assembly with a strike pattern, the real landing
///   function run with step 0 in flight: the landing counter moves by one, the
///   machine returns to idle and the step index advances.
///
/// A PackTest dummy is permanently undamageable (see `lethal_<id>`'s own
/// note), so what a landing does to a player's health is the bot tier's to
/// witness; this suite proves the machine that delivers it.
pub(super) fn emit_assembly_packtests(
    plan: &Plan,
    locks: &crate::compiler::assembly::Locks,
    out: &mut BuildOutput,
) {
    use crate::compiler::assembly as asm;
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let write = |name: &str, b: Vec<String>, out: &mut BuildOutput| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&b).into_bytes(),
        );
    };
    for p in asm::placed(plan) {
        let s = p.safe.clone();
        let id = p.decl.id.as_str();
        let reset = [
            format!("kill @e[tag={}]", asm::tag(&s)),
            format!(
                "scoreboard players set {} dw.sys 0",
                asm::holder(&s, "live")
            ),
        ];

        // --- asm_spawn_<s> ---
        let mut b = packtest_header(&format!(
            "{title}: assembly `{id}` spawns its root, {} part(s) riding it and its hitbox, and \
             leaves nothing behind when it despawns (spec-0082)",
            p.rig.parts.len()
        ));
        b.push(format!("function {ns}:setup"));
        b.extend(reset.iter().cloned());
        b.push(format!("function {ns}:{}", asm::spawn_fn(&s)));
        let n = format!("#asmn_{s}");
        b.push(format!("scoreboard players set {n} dw.sys 0"));
        b.push(format!(
            "execute as @e[tag={},limit=1] on passengers run scoreboard players add {n} dw.sys 1",
            asm::root_tag(&s)
        ));
        b.push(format!(
            "assert score {n} dw.sys matches {}",
            p.rig.parts.len()
        ));
        let h = format!("#asmh_{s}");
        b.push(format!(
            "execute store result score {h} dw.sys if entity @e[type=minecraft:interaction,tag={}]",
            asm::hit_tag(&s)
        ));
        b.push(format!(
            "assert score {h} dw.sys matches {}",
            usize::from(p.decl.hitbox.is_some())
        ));
        b.push(format!("function {ns}:{}", asm::despawn_fn(&s)));
        let left = format!("#asme_{s}");
        b.push(format!(
            "execute store result score {left} dw.sys if entity @e[tag={}]",
            asm::tag(&s)
        ));
        b.push(format!("assert score {left} dw.sys matches 0"));
        write(&format!("asm_spawn_{s}"), b, out);

        // --- asm_hits_<s>_<trigger> ---
        for t in &plan.campaign.quests.content.triggers {
            if t.on.assembly_target().map(|a| a.as_str()) != Some(id) || p.decl.hitbox.is_none() {
                continue;
            }
            // The counted datum: the bundle's first ungated `add-state` on a
            // party datum.
            let Some((datum, amount)) = t.effects.iter().find_map(|e| match &e.verb {
                Verb::AddState { state, amount }
                    if e.when.is_none()
                        && plan
                            .campaign
                            .quests
                            .content
                            .state_decl(state.as_str())
                            .is_some_and(|d| d.scope == StateScope::Party) =>
                {
                    Some((state, *amount))
                }
                _ => None,
            }) else {
                continue;
            };
            let score = plan::state_score(datum.as_str());
            let tsafe = plan::safe_local(t.id.as_str());
            let initial = plan
                .campaign
                .quests
                .content
                .state_decl(datum.as_str())
                .map(|d| d.initial)
                .unwrap_or(0);
            // The trigger's own gate, owned by the template (`DW0807`).
            let mut own: Vec<String> = Vec::new();
            for f in &t.requires_flags {
                own.push(format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    plan::flag_score(f.as_str())
                ));
            }
            for f in &t.forbids_flags {
                own.push(format!(
                    "scoreboard players set {} {} 0",
                    plan::PARTY,
                    plan::flag_score(f.as_str())
                ));
            }
            own.push(format!("scoreboard players set #trig_{tsafe} dw.sys 0"));
            let hit = format!(
                "data merge entity @e[tag={},limit=1] {{attack:{{player:[I;0,0,0,1],timestamp:0L}}}}",
                asm::hit_tag(&s)
            );
            let mut b = packtest_header(&format!(
                "{title}: a blow on assembly `{id}`'s hitbox fires `{}` and moves `{}` by {amount} \
                 (spec-0082)",
                t.id, datum
            ));
            b.push(format!("function {ns}:setup"));
            // The blow meets the very clause `tick` polls it with, and the
            // clear after it — never the whole `tick`, whose other gates read
            // the whole progression ledger, and a template that zeroes that
            // ledger inline runs under the campaign template's own phases in
            // the same batch. What the clause reads is the trigger's own gate
            // (`DW0807`), which `own` writes.
            let (poll, clear) = click_trigger_poll(plan, t);
            b.extend(reset.iter().cloned());
            b.push(format!("function {ns}:{}", asm::spawn_fn(&s)));
            b.extend(own.iter().cloned());
            b.push(format!(
                "scoreboard players set {} {score} {initial}",
                plan::PARTY
            ));
            b.push(hit.clone());
            b.push(poll.clone());
            b.push(clear.clone());
            b.push(format!(
                "assert score {} {score} matches {}",
                plan::PARTY,
                i64::from(initial) + i64::from(amount)
            ));
            // The clip the count plays, where the bundle waits on the datum.
            let counted = t.effects.iter().find_map(|e| match &e.verb {
                Verb::PlayClip { assembly, clip } if assembly.as_str() == id => e
                    .requires_state()
                    .iter()
                    .find(|c| c.state == *datum && c.op == delvewright_dsl::CompareOp::AtLeast)
                    .and_then(|c| p.rig.clip_index(clip).map(|k| (c.value, k))),
                _ => None,
            });
            if let Some((at, k)) = counted {
                b.extend(own.iter().cloned());
                b.push(format!(
                    "scoreboard players set {} dw.sys 0",
                    asm::holder(&s, "sm")
                ));
                b.push(format!(
                    "scoreboard players set {} {score} {}",
                    plan::PARTY,
                    i64::from(at) - i64::from(amount)
                ));
                b.push(hit.clone());
                b.push(poll.clone());
                b.push(clear.clone());
                b.push(format!("assert score {} {score} matches {at}", plan::PARTY));
                b.push(format!(
                    "assert score {} dw.sys matches {k}",
                    asm::holder(&s, "base")
                ));
                b.push(format!(
                    "assert score {} dw.sys matches {k}",
                    asm::holder(&s, "clip")
                ));
            }
            b.push(format!("function {ns}:{}", asm::despawn_fn(&s)));
            b.extend(own.iter().cloned());
            b.push(format!(
                "scoreboard players set {} {score} {initial}",
                plan::PARTY
            ));
            write(&format!("asm_hits_{s}_{tsafe}"), b, out);
        }

        // --- asm_land_<s> ---
        let Some(st) = p.decl.strikes.as_ref().filter(|st| !st.pattern.is_empty()) else {
            continue;
        };
        let lands = asm::holder(&s, "lands");
        let mut b = packtest_header(&format!(
            "{title}: assembly `{id}`'s strike lands, counts the landing, and returns the machine \
             to idle (spec-0082)"
        ));
        b.push(format!("function {ns}:setup"));
        b.extend(reset.iter().cloned());
        b.push(format!("function {ns}:{}", asm::spawn_fn(&s)));
        b.push(format!("scoreboard players set {lands} dw.sys 0"));
        b.push(format!(
            "scoreboard players set {} dw.sys 3",
            asm::holder(&s, "sm")
        ));
        b.push(format!(
            "scoreboard players set {} dw.sys 0",
            asm::holder(&s, "step")
        ));
        b.push(format!("function {ns}:{}", asm::land_fn(&s)));
        b.push(format!("assert score {lands} dw.sys matches 1"));
        b.push(format!(
            "assert score {} dw.sys matches 0",
            asm::holder(&s, "sm")
        ));
        b.push(format!(
            "assert score {} dw.sys matches {}",
            asm::holder(&s, "step"),
            usize::from(st.pattern.len() > 1)
        ));
        b.push(format!("function {ns}:{}", asm::despawn_fn(&s)));
        write(&format!("asm_land_{s}"), b, out);

        // --- asm_hold_<s> (spec-0094 §3.3) ---
        // A clip the story plays stands the pattern down: with a body in the
        // region that arms step 0, the real tick begins no wind-up over it;
        // the real `arm-strikes` re-arms, and the next tick begins one.
        let Some(arming) = plan.zone_box(&st.while_in) else {
            continue;
        };
        let region = st.pattern[0]
            .lock
            .as_ref()
            .and_then(|l| plan.zone_box(&l.within))
            .unwrap_or(arming);
        let Some(cue) = p
            .decl
            .initial
            .as_deref()
            .and_then(|c| p.rig.clip_index(c))
            .or_else(|| (!p.rig.clips.is_empty()).then_some(0))
        else {
            continue;
        };
        let (pin, me) = pin_dummy(&format!("dw_asm_hold_{s}"));
        let mut b = packtest_header(&format!(
            "{title}: a clip the story plays on assembly `{id}` holds while a body stands in \
             its arming region, and only `arm-strikes` begins the next wind-up (spec-0094)"
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.extend(reset.iter().cloned());
        b.push(format!("function {ns}:{}", asm::spawn_fn(&s)));
        b.push(format!(
            "tp {me} {} {} {}",
            f64::from(region.0[0] + region.1[0]) / 2.0 + 0.5,
            region.0[1],
            f64::from(region.0[2] + region.1[2]) / 2.0 + 0.5
        ));
        b.push(format!(
            "scoreboard players set {} dw.sys 0",
            asm::holder(&s, "sm")
        ));
        b.push(format!(
            "scoreboard players set {} dw.sys 0",
            asm::holder(&s, "step")
        ));
        b.push(format!("function {ns}:{}", asm::cue_fn(&s, cue)));
        b.push(format!("function {ns}:{}", asm::tick_fn(&s)));
        b.push(format!(
            "assert score {} dw.sys matches 0",
            asm::holder(&s, "sm")
        ));
        b.push(format!(
            "assert score {} dw.sys matches {cue}",
            asm::holder(&s, "clip")
        ));
        b.push(format!("function {ns}:{}", asm::arm_fn(&s)));
        b.push(format!("function {ns}:{}", asm::tick_fn(&s)));
        b.push(format!(
            "assert score {} dw.sys matches 1",
            asm::holder(&s, "sm")
        ));
        b.push(format!("function {ns}:{}", asm::despawn_fn(&s)));
        b.push(format!("tag {me} remove dw_asm_hold_{s}"));
        write(&format!("asm_hold_{s}"), b, out);

        // --- asm_lock_<s>_<j> (spec-0094 §4.2) ---
        // The lock's dispatch, driven with the cells the plan proved: the
        // feet cell of the first and of the most turned proved cell, written
        // where the choice writes it, resolve to that cell — the root turned to
        // its yaw, the pose and the landing it was proved with.
        for (j, _) in st.pattern.iter().enumerate() {
            let Some(lp) = locks.get(&(p.index, j)) else {
                continue;
            };
            let Some(within) = lp.within else { continue };
            if lp.cells.is_empty() {
                continue;
            }
            let far = (0..lp.cells.len())
                .max_by(|a, b| {
                    lp.cells[*a]
                        .yaw
                        .abs()
                        .total_cmp(&lp.cells[*b].yaw.abs())
                        .then(b.cmp(a))
                })
                .unwrap_or(0);
            let mut b = packtest_header(&format!(
                "{title}: assembly `{id}`'s strike step {j} turns to the cell it locks onto and \
                 takes the pose it was proved with there (spec-0094)"
            ));
            b.push(format!("function {ns}:setup"));
            b.extend(reset.iter().cloned());
            b.push(format!("function {ns}:{}", asm::spawn_fn(&s)));
            let qs = if far == 0 { vec![0] } else { vec![0, far] };
            for q in qs {
                let cell = &lp.cells[q];
                for (a, axis) in ["l0", "l1", "l2"].iter().enumerate() {
                    b.push(format!(
                        "data modify storage {} {s}.{axis} set value {}",
                        asm::STORAGE,
                        cell.cell[a] - within.0[a]
                    ));
                }
                b.push(format!(
                    "function {ns}:asm_lockat_{s}_{j} with storage {} {s}",
                    asm::STORAGE
                ));
                let got = format!("#asmq_{s}");
                b.push(format!(
                    "execute store result score {got} dw.sys run data get storage {} {s}.q",
                    asm::STORAGE
                ));
                b.push(format!("assert score {got} dw.sys matches {q}"));
                let yaw = format!("#asmy_{s}");
                b.push(format!(
                    "execute store result score {yaw} dw.sys run data get entity @e[tag={},limit=1] Rotation[0] 10",
                    asm::root_tag(&s)
                ));
                let want = (cell.yaw * 10.0).floor() as i64;
                b.push(format!(
                    "assert score {yaw} dw.sys matches {}..{}",
                    want - 1,
                    want + 1
                ));
            }
            b.push(format!("function {ns}:{}", asm::despawn_fn(&s)));
            write(&format!("asm_lock_{s}_{j}"), b, out);
        }
    }
}
