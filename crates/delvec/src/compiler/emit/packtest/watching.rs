use super::*;

use crate::compiler::watching::{WATCH_FN, WATCH_TAG, WatchBinding, Watcher, y_rotation_range};

/// The tolerance, in degrees, a generated PackTest reads a body's yaw within.
const YAW_TOLERANCE: f64 = 1.0;

/// spec-0101 §5.3: per watching body, `watch_<class>_<id>` — the body is
/// summoned through its own entrance, the template's dummy stands on the
/// drawable cell whose bearing differs most from the body's home facing, one
/// real `tick` turns the body to face it (`y_rotation` within ±1° of the game's
/// bearing), and with the dummy moved beyond `within` a second `watch_tick`
/// leaves the yaw where it was. A class watch first shows the dummy drawing
/// nothing until it wears the class. Per watching body with a walk,
/// `watch_yield_<class>_<id>`: each walk's start removes the live-watch tag
/// and its arrival tick restores it.
///
/// **Every other player is out of the watch for the template's length.** `@p`
/// is the nearest player, and the suite runs as one batch: a sibling's dummy
/// left standing nearer the body would be the one it turns to. Each template
/// tags every other player `dw_cutscene` — the exclusion the watch line carries —
/// and clears exactly the tags it added before its assertions. `#cs_live` is
/// held at 1 across the same lines, so the real `tick` does not repair the
/// players it hid as stranded cutscene viewers, and is put back after. Each
/// template is one atomic function, so no sibling observes either.
///
/// A watcher the drawability proof found no observable cell for (`unobservable`
/// in the binding line) gets no turn template: a turn smaller than the
/// observable margin is not a turn a test could tell from none.
pub(super) fn emit_watch_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
    binding: &WatchBinding,
) {
    let ns = &plan.namespace;
    for (k, r) in binding.records.iter().enumerate() {
        let w = &r.watcher;
        let name = template_stem(w);
        if let Some((cell, yaw)) = r.test {
            let b = turn_template(plan, w, k, cell, yaw);
            out.insert(
                format!("packtest-datapack/data/{ns}/test/watch_{name}.mcfunction"),
                lines(&b).into_bytes(),
            );
        }
        if let Some(b) = yield_template(plan, w, k, moves, actor_moves) {
            out.insert(
                format!("packtest-datapack/data/{ns}/test/watch_yield_{name}.mcfunction"),
                lines(&b).into_bytes(),
            );
        }
    }
}

/// `npc_<safe>` / `actor_<safe>`: the body's class and local id, so an npc and
/// an actor that share a local name never share a template.
fn template_stem(w: &Watcher) -> String {
    format!("{}_{}", w.class, plan::safe_local(&w.id))
}

/// The lines that summon the body fresh, through its own entrance: an npc by
/// the world-init summon (or its `spawn_npc_<id>` when `deferred`), a puppet by
/// its `spawn_actor_<id>`.
fn summon_body(plan: &Plan, w: &Watcher) -> Vec<String> {
    let ns = &plan.namespace;
    let mut b = Vec::new();
    match w.class {
        "npc" => {
            b.push(format!("function {ns}:setup"));
            b.push("scoreboard players set #placed dw.sys 1".to_string());
            // Clear every planned NPC tag before re-running the unguarded
            // `setup_finish`, or a duplicated body would stand beside this one.
            for n in &plan.npcs {
                b.push(format!("kill @e[tag={}]", n.tag));
            }
            b.push(format!("function {ns}:setup_finish"));
            if npc_is_deferred(plan.campaign, &w.id) {
                b.push(format!("function {ns}:{}", spawn_npc_fn(&w.id)));
            }
        }
        _ => {
            let safe = plan::safe_local(&w.id);
            b.push(format!("kill @e[tag=dw_actor_{safe}]"));
            b.push(format!("function {ns}:spawn_actor_{safe}"));
        }
    }
    b
}

/// The lines that put the body back as its entrance leaves it, so a template
/// leaves no residue of its own behind.
fn restore_body(plan: &Plan, w: &Watcher) -> Vec<String> {
    let ns = &plan.namespace;
    match w.class {
        "npc" => {
            let mut b: Vec<String> = plan
                .npcs
                .iter()
                .map(|n| format!("kill @e[tag={}]", n.tag))
                .collect();
            b.push(format!("function {ns}:setup_finish"));
            b
        }
        _ => vec![format!("kill @e[tag=dw_actor_{}]", plan::safe_local(&w.id))],
    }
}

/// Count the bodies the watcher's selector matches facing `yaw` ± the
/// template tolerance into `holder`.
fn facing_count(w: &Watcher, yaw: f64, holder: &str) -> String {
    format!(
        "execute store result score {holder} dw.sys if entity @e[{},y_rotation={}]",
        w.selector,
        y_rotation_range(yaw, YAW_TOLERANCE)
    )
}

fn turn_template(plan: &Plan, w: &Watcher, k: usize, cell: [i32; 3], yaw: f64) -> Vec<String> {
    let ns = &plan.namespace;
    let me = format!("dw_wt_{k}");
    let hid = format!("dw_wh_{k}");
    let stand = crate::compiler::nav::cell_center(cell);
    let classed = w
        .class_tag
        .as_ref()
        .map(|t| format!(" wearing `{t}`"))
        .unwrap_or_default();
    let mut b = packtest_header(&format!(
        "{}: `{}` watches — it turns to face a player{classed} within {} block(s) at [{}, {}, {}] \
         (yaw {:.2}, home {:.2}) and holds that facing once the player is out of reach",
        artifact_title(plan.campaign),
        w.id,
        w.within,
        cell[0],
        cell[1],
        cell[2],
        yaw,
        w.home_yaw
    ));
    b.push(format!("tag @s add {me}"));
    b.extend(summon_body(plan, w));
    // Every other player out of the watch, and the cutscene repair held off them.
    b.push(format!("tag @a[tag=!{me},tag=!dw_cutscene] add {hid}"));
    b.push(format!("tag @a[tag={hid}] add dw_cutscene"));
    b.push("tag @s remove dw_cutscene".to_string());
    b.push(format!(
        "execute store result score #wcl_{k} dw.sys run scoreboard players get #cs_live dw.sys"
    ));
    b.push("scoreboard players set #cs_live dw.sys 1".to_string());
    // The dummy is placed, never first-joined: the real tick must not move it.
    b.push("tag @s add dw_joined".to_string());
    if let Some(t) = &w.class_tag {
        b.push(format!("tag @s remove {t}"));
    }
    b.push(format!(
        "tp @s {} {} {}",
        fmt_f64(stand[0]),
        fmt_f64(stand[1]),
        fmt_f64(stand[2])
    ));
    // 1. the body stands at its home facing before any watch runs
    b.push(facing_count(w, w.home_yaw, &format!("#wh_{k}")));
    // 2. a class watch draws nothing from a player without the class
    if let Some(t) = &w.class_tag {
        b.push(format!("function {ns}:{WATCH_FN}"));
        b.push(facing_count(w, w.home_yaw, &format!("#wc_{k}")));
        b.push(format!("tag @s add {t}"));
    }
    // 3. one real tick turns the body to face the dummy
    b.extend(shielded_tick(ns));
    b.push(facing_count(w, yaw, &format!("#wt_{k}")));
    // 4. out of reach, the body keeps its last facing
    b.push(format!(
        "tp @s {} {} {}",
        fmt_f64(stand[0]),
        fmt_f64(stand[1] + f64::from(w.within) + 3.0),
        fmt_f64(stand[2])
    ));
    b.push(format!("function {ns}:{WATCH_FN}"));
    b.push(facing_count(w, yaw, &format!("#wk_{k}")));
    // Put back everything this template changed before anything is asserted.
    b.push(format!(
        "scoreboard players operation #cs_live dw.sys = #wcl_{k} dw.sys"
    ));
    b.push(format!("tag @a[tag={hid}] remove dw_cutscene"));
    b.push(format!("tag @a[tag={hid}] remove {hid}"));
    if let Some(t) = &w.class_tag {
        b.push(format!("tag @s remove {t}"));
    }
    b.extend(restore_body(plan, w));
    b.push(format!("assert score #wh_{k} dw.sys matches 1"));
    if w.class_tag.is_some() {
        b.push(format!("assert score #wc_{k} dw.sys matches 1"));
    }
    b.push(format!("assert score #wt_{k} dw.sys matches 1"));
    b.push(format!("assert score #wk_{k} dw.sys matches 1"));
    b
}

fn yield_template(
    plan: &Plan,
    w: &Watcher,
    k: usize,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) -> Option<Vec<String>> {
    let ns = &plan.namespace;
    let safe = plan::safe_local(&w.id);
    // (start function, latch, tick counter, tick function, total, claim)
    let mut legs: Vec<(String, String, String, String, usize, Vec<String>)> = Vec::new();
    match w.class {
        "npc" => {
            let guarded = supersedable_walkers(moves.iter().map(|m| m.npc.as_str()));
            for m in moves.iter().filter(|m| m.npc == w.id) {
                let bare = movenpc_bare(&m.npc, &m.to, &m.gate_key);
                legs.push((
                    movenpc_fn(&m.npc, &m.to, &m.gate_key),
                    format!("#mrun_{bare}"),
                    format!("#mt_{bare}"),
                    format!("mv_tick_{bare}"),
                    m.ticks(),
                    walk_claim(Walker::Npc, &bare, &safe, guarded.contains(m.npc.as_str())),
                ));
            }
        }
        _ => {
            let guarded = supersedable_walkers(actor_moves.iter().map(|m| m.actor.as_str()));
            for m in actor_moves.iter().filter(|m| m.actor == w.id) {
                let bare = moveactor_bare(&m.actor, &m.to, &m.gate_key);
                legs.push((
                    moveactor_fn(&m.actor, &m.to, &m.gate_key),
                    format!("#arun_{bare}"),
                    format!("#at_{bare}"),
                    format!("ma_tick_{bare}"),
                    m.ticks(),
                    walk_claim(
                        Walker::Actor,
                        &bare,
                        &safe,
                        guarded.contains(m.actor.as_str()),
                    ),
                ));
            }
        }
    }
    if legs.is_empty() {
        return None;
    }
    let mut b = packtest_header(&format!(
        "{}: `{}` yields its watch to each of its {} walk(s) — the start removes `{WATCH_TAG}`, \
         the arrival tick restores it",
        artifact_title(plan.campaign),
        w.id,
        legs.len()
    ));
    b.extend(summon_body(plan, w));
    let mut asserts = Vec::new();
    for (i, (start, latch, counter, tick, total, claim)) in legs.into_iter().enumerate() {
        // A latch a sibling left armed would refuse the start.
        b.push(format!("scoreboard players set {latch} dw.sys 0"));
        b.extend(claim);
        b.push(format!("function {ns}:{start}"));
        b.push(format!(
            "execute store result score #wya_{k}_{i} dw.sys if entity @e[{},tag={WATCH_TAG}]",
            w.selector
        ));
        b.push(format!("scoreboard players set {counter} dw.sys {total}"));
        b.push(format!("function {ns}:{tick}"));
        b.push(format!(
            "execute store result score #wyp_{k}_{i} dw.sys if entity @e[{},tag={WATCH_TAG}]",
            w.selector
        ));
        asserts.push(format!("assert score #wya_{k}_{i} dw.sys matches 0"));
        asserts.push(format!("assert score #wyp_{k}_{i} dw.sys matches 1"));
    }
    b.extend(restore_body(plan, w));
    b.extend(asserts);
    Some(b)
}
