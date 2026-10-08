//! Timed gates (spec-0016 §4).

use super::*;

/// `setup_finish` commands for timed gates (spec-0016 §4): start each gate's
/// clock, and summon the disarm affordance of any gate that
/// declares one. The gate is physically sealed by the prefab at world-load, so
/// the clock's first act is always an OPEN — a `phase` of 0 opens immediately, a
/// larger one holds the gate shut that many ticks first. Empty for a campaign
/// with no timed gate.
///
/// The affordance is the same pair a shortcut unlock and a trap disarm emit: an
/// invisible `minecraft:interaction` hitbox **plus** compiler-owned visible
/// hardware, because a hitbox alone is a lever the player cannot see — the
/// drowned-bell soft-lock class `DW0420` exists to make impossible.
pub(super) fn timed_gate_setup(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for g in &plan.timed_gates {
        if g.phase == 0 {
            out.push(format!("function {ns}:tgate_open_{}", g.safe));
        } else {
            out.push(format!(
                "schedule function {ns}:tgate_open_{} {}t",
                g.safe, g.phase
            ));
        }
    }
    for g in &plan.timed_gates {
        let Some(dis) = &g.disarm else {
            continue;
        };
        let v = ent_xyz(dis.via_cell);
        out.push(format!(
            "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"dw_tgdis_{}\"]}}",
            v[0], v[1], v[2], g.safe
        ));
        out.push(affordance_hardware(
            v,
            &format!("dw_tgdis_{}", g.safe),
            "minecraft:lever",
        ));
    }
    out
}

/// Per-tick disarm detection for jammable timed gates, reusing the
/// v0.4 interaction-entity `use` primitive exactly as a trap disarm and a
/// shortcut unlock do. The `#tgdis_<id>` sentinel makes the jam fire **once**;
/// after it, there is nothing left to dispatch. Empty for a campaign with no
/// disarmable gate → byte-identical.
pub(super) fn timed_gate_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for g in &plan.timed_gates {
        if g.disarm.is_none() {
            continue;
        }
        let id = &g.safe;
        out.push(format!(
            "execute unless score #tgdis_{id} dw.sys matches 1 if entity @e[tag=dw_tgdis_{id},nbt={{interaction:{{}}}}] run function {ns}:tgate_disarm_{id}"
        ));
        out.push(format!(
            "execute as @e[tag=dw_tgdis_{id}] run data remove entity @s interaction"
        ));
    }
    out
}

/// Half-hearts dealt by a `crush: true` timed gate's closing edge (spec-0016 §4
/// addendum). Far above any reachable effective health: `minecraft:generic`
/// ignores armor but not absorption/resistance, and the point of a portcullis
/// is that being caught in it is not survivable by gearing.
pub(super) const CRUSH_DAMAGE: u32 = 1000;

/// The timed-gate clock functions (spec-0016 §4): a two-function ping-pong that
/// carries its own next hop, so the cycle is one self-sustaining chain with no
/// per-tick polling and no state to drift.
///
/// `tgate_open_<id>` clears the region (the same `fill … replace <block>`
/// `open-gate` emits) and schedules the close `open_ticks` later;
/// `tgate_close_<id>` fills it back and schedules the open `closed_ticks` later.
/// `schedule` is replace-mode in vanilla, so the clock can never double up — the
/// same property the boundary and night-vision clocks rely on. Both functions are
/// pure world edits: they name no player, so the server command source they are
/// re-entered under is irrelevant (§4 "A scheduled bundle has no `@s`").
pub(super) fn emit_timed_gate_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for g in &plan.timed_gates {
        let id = &g.safe;
        let (from, to) = g.gate_region;
        // The jam guard. A disarmable gate's clock lines are prefixed
        // with `execute unless score #tgdis_<id> dw.sys matches 1` — the same
        // score-guard shape a gated trap's payload uses. A gate with no `disarm`
        // emits no guard at all, so its output is byte-identical to before.
        let guard = if g.disarm.is_some() {
            Some(format!("execute unless score #tgdis_{id} dw.sys matches 1"))
        } else {
            None
        };
        // `<guard> run <cmd>` when jammable, else `<cmd>` verbatim.
        let guarded = |cmd: String| match &guard {
            Some(g) => format!("{g} run {cmd}"),
            None => cmd,
        };
        // …and the `execute` form, for a line that is already an `execute`: its
        // subcommands splice straight onto the guard rather than nesting.
        let guarded_exec = |rest: &str, cmd: &str| match &guard {
            Some(g) => format!("{g} {rest} run {cmd}"),
            None => format!("execute {rest} run {cmd}"),
        };
        out.push((
            format!("tgate_open_{id}"),
            lines(&[
                // The open itself is NEVER guarded: a jam that lands while the
                // gate is shut leaves one already-scheduled open in flight, and
                // that open is exactly what parks the portcullis in its resting
                // position. Suppressing it would freeze the gate CLOSED, which
                // is the opposite of a disarm.
                format!(
                    "fill {} {} {} {} {} {} minecraft:air replace {}",
                    from[0], from[1], from[2], to[0], to[1], to[2], g.gate_block
                ),
                guarded(format!(
                    "schedule function {ns}:tgate_close_{id} {}t",
                    g.open_ticks
                )),
            ]),
        ));
        let mut close = Vec::new();
        // spec-0016 §4 addendum: the portcullis judgement. Emitted BEFORE the
        // fill so the victim is judged on the world as it was when they
        // mistimed it — after the fill they are already inside a solid block
        // and vanilla's own suffocation would be the thing killing them, which
        // is slow, gear-dependent and escapable. Costs nothing per tick: this
        // rides the closing tick of the schedule ping-pong that already exists.
        //
        // The judgement sits INSIDE the suppressed clock, so a
        // disarmed gate can never crush — there is no closing tick left to be
        // caught by. That is not a second rule, it is the same guard.
        if g.crush {
            close.push(guarded_exec(
                &format!("as @a[{}]", region_selector(from, to)),
                &format!("damage @s {CRUSH_DAMAGE} minecraft:generic"),
            ));
        }
        close.push(guarded(format!(
            "fill {} {} {} {} {} {} {}",
            from[0], from[1], from[2], to[0], to[1], to[2], g.gate_block
        )));
        close.push(guarded(format!(
            "schedule function {ns}:tgate_open_{id} {}t",
            g.closed_ticks
        )));
        out.push((format!("tgate_close_{id}"), lines(&close)));
    }
    out.extend(emit_timed_gate_disarm_functions(plan));
    out
}

/// The `tgate_disarm_<id>` functions: jam the gate for good.
///
/// Four commands, and the ORDER is the semantics:
/// 1. latch `#tgdis_<id>` — from this instant every guarded clock line is inert,
///    including the crush;
/// 2. raise the disarm flag party-wide, so the rest of the campaign can read
///    "the party switched it off" (`requires_flags`, a dialogue gate, a quest);
/// 3. clear the span **once** — the jammed portcullis comes to rest OPEN, which
///    is what a disarm means and what a player who pulls a lever expects to see;
/// 4. retire the affordance's visible hardware. This is the ONE function allowed
///    to do that — `DW0421` fails the build if anything else reaches it.
///
/// There is deliberately **no** `schedule clear`. A close already in flight fires
/// into the guard and does nothing — including not scheduling the next open — so
/// the ping-pong dies of its own accord within one hop, and the gate is left open
/// by step 3. Clearing a schedule that may not exist would be a command that
/// fails at runtime for no gain.
pub(super) fn emit_timed_gate_disarm_functions(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for g in &plan.timed_gates {
        let Some(dis) = &g.disarm else {
            continue;
        };
        let id = &g.safe;
        let (from, to) = g.gate_region;
        let body = vec![
            format!("scoreboard players set #tgdis_{id} dw.sys 1"),
            format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                plan::flag_score(&dis.sets_flag)
            ),
            format!(
                "fill {} {} {} {} {} {} minecraft:air replace {}",
                from[0], from[1], from[2], to[0], to[1], to[2], g.gate_block
            ),
            format!(
                "kill @e[tag={}]",
                crate::compiler::affordance::hardware_tag(&format!("dw_tgdis_{id}"))
            ),
        ];
        out.push((format!("tgate_disarm_{id}"), lines(&body)));
    }
    out
}
