use super::*;

use crate::compiler::emit::moves::{HOP_OBJECTIVE, HOP_TICK_FN};
use delvewright_dsl::{BodyKey, HOP_UNIT, hop_arc, hop_decimal};

/// spec-0136 PackTests — **a declared hop moves its body and returns it**:
/// per `hop` effect the campaign declares, `hop_<n>`, over the first body it
/// names that the plan places.
///
/// The body is summoned through its own entrance and its height read. The
/// hop is then started by **the effect's own emitted line** for that body
/// (`emit_move`, never a restatement) and walked by the pack's own driver,
/// `hop_tick`, called once per tick of the arc — not by `tick`, which would
/// run the campaign's content with it; that the root `tick` calls `hop_tick`
/// is the compiler's own test. Three readings, each exact, in
/// [`HOP_UNIT`]ths of a block (`data get … Pos[1] <unit>`):
///
/// * after the start, the body counts the arc's ticks — the hop was taken;
/// * on the arc's peak tick, the body stands the arc's peak above where it
///   stood — it moved, by the declared height;
/// * after the last tick, it stands on the height it left, and counts 0 — it
///   came back.
///
/// A hop that never fires — a start line that selects nobody, a driver that
/// steps nobody — fails the first or the second reading. The template is one
/// atomic function with its own score holders, so no sibling is interleaved;
/// it zeroes the walk latches the start line asks, which are shared runtime
/// scores a sibling may have left set.
pub(super) fn emit_hop_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let title = artifact_title(c);
    let mut n = 0usize;
    for eff in all_campaign_effects(c) {
        let Verb::Hop { height } = &eff.verb else {
            continue;
        };
        let Some(key) = eff
            .bodies()
            .into_iter()
            .find(|k| k.resolve(c).is_some_and(|b| plan.body_point(b).is_some()))
        else {
            continue; // no placed body: `DW0112` and the placement rules own it
        };
        let arc = hop_arc(*height);
        let peak = arc.iter().copied().max().unwrap_or(0);
        let peak_tick = arc.iter().position(|h| *h == peak).unwrap_or(0) + 1;
        let body = body_sel(&key, true);
        // One holder per reading, so a red names the reading that failed.
        let (base, now, flag, landed) = (
            format!("#hopb{n}"),
            format!("#hopy{n}"),
            format!("#hopt{n}"),
            format!("#hopl{n}"),
        );
        let read = |holder: &str| {
            format!(
                "execute store result score {holder} dw.sys run data get entity {body} Pos[1] {HOP_UNIT}"
            )
        };

        let mut b = packtest_header(&format!(
            "{title}: the hop of {} `{}` rises {} block(s) and lands where it stood",
            key.class(),
            key.id(),
            hop_decimal(peak)
        ));
        // The body, fresh, through its own entrance.
        match &key {
            BodyKey::Npc(_) => {
                b.extend(settled_setup(ns));
                for npc in &plan.npcs {
                    b.push(format!("kill @e[tag={}]", npc.tag));
                }
                b.push(format!("function {ns}:setup_finish"));
                if npc_is_deferred(c, key.id()) {
                    b.push(format!("function {ns}:{}", spawn_npc_fn(key.id())));
                }
            }
            BodyKey::Actor(_) => {
                let safe = plan::safe_local(key.id());
                b.push(format!("kill @e[tag=dw_actor_{safe}]"));
                b.push(format!("function {ns}:spawn_actor_{safe}"));
            }
        }
        for latch in walk_latches(c, &key) {
            b.push(format!("scoreboard players set {latch} dw.sys 0"));
        }
        b.push(read(&base));

        // The start: the effect's own line, for this body alone.
        let mut one = eff.clone();
        one.repeat = None;
        one.stagger_ticks = None;
        one.audience = Some(delvewright_dsl::EffectAudience::Bodies(vec![key.clone()]));
        emit_move(plan, &one, &mut b);
        b.push(format!("scoreboard players set {flag} dw.sys 0"));
        b.push(format!(
            "execute as {body} if score @s {HOP_OBJECTIVE} matches {} run scoreboard players set {flag} dw.sys 1",
            arc.len()
        ));
        b.push(format!("assert score {flag} dw.sys matches 1"));

        // Up: the driver, to the arc's peak tick.
        for _ in 0..peak_tick {
            b.push(format!("function {ns}:{HOP_TICK_FN}"));
        }
        b.push(read(&now));
        b.push(format!(
            "scoreboard players operation {now} dw.sys -= {base} dw.sys"
        ));
        b.push(format!("assert score {now} dw.sys matches {peak}"));

        // Down: the rest of the arc.
        for _ in peak_tick..arc.len() {
            b.push(format!("function {ns}:{HOP_TICK_FN}"));
        }
        b.push(read(&now));
        b.push(format!(
            "scoreboard players operation {now} dw.sys -= {base} dw.sys"
        ));
        b.push(format!("assert score {now} dw.sys matches 0"));
        b.push(format!("scoreboard players set {landed} dw.sys 0"));
        b.push(format!(
            "execute as {body} if score @s {HOP_OBJECTIVE} matches 0 run scoreboard players set {landed} dw.sys 1"
        ));
        b.push(format!("assert score {landed} dw.sys matches 1"));

        // No residue: the body as its entrance leaves it.
        match &key {
            BodyKey::Npc(_) => {
                for npc in &plan.npcs {
                    b.push(format!("kill @e[tag={}]", npc.tag));
                }
                b.push(format!("function {ns}:setup_finish"));
            }
            BodyKey::Actor(_) => {
                b.push(format!(
                    "kill @e[tag=dw_actor_{}]",
                    plan::safe_local(key.id())
                ));
            }
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/hop_{n}.mcfunction"),
            lines(&b).into_bytes(),
        );
        n += 1;
    }
}
