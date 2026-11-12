//! A body the story moves (spec-0136): the commands of a staged body's moves,
//! the hop driver, and the functions of an effect that keeps a cadence.
//!
//! Every primitive here was measured on the pinned server and is recorded in
//! `docs/reference/npc-moves.md`; the section each line rests on is named
//! where it is written.

use super::*;
use delvewright_dsl::{BodyKey, FaceTo, hop_arc, hop_decimal};

/// A number as a command argument takes it: a whole number bare (`20`), any
/// other as its shortest decimal (`1.25`).
fn num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// The objective a hopping body counts its remaining ticks in.
pub(crate) const HOP_OBJECTIVE: &str = "dw.hop";
/// The objective a hopping body carries its arc's index in.
pub(crate) const HOP_ARC_OBJECTIVE: &str = "dw.hop_arc";
/// The function the root `tick` calls: it runs [`HOP_STEP_FN`] as every body
/// in the air. One function of its own, so the hop's generated PackTest can
/// drive the hop by the driver the pack ships and nothing else of `tick`.
pub(crate) const HOP_TICK_FN: &str = "hop_tick";
/// The function [`HOP_TICK_FN`] runs as every body in the air.
pub(crate) const HOP_STEP_FN: &str = "hop_step";

/// The selector terms that address one staged body and nothing else.
///
/// `staged` is the body as the story stood it: an NPC's body (never its click
/// box) or an actor's puppet (never the twin an `unleash-actor` frees, which
/// moves by its own AI). Not `staged` is the body whoever wears it now — an
/// actor's puppet or its twin — for a move that sets no position and no
/// facing: an item, a size, a place for a particle or a sound.
pub(super) fn body_terms(key: &BodyKey, staged: bool) -> String {
    let safe = plan::safe_local(key.id());
    match key {
        BodyKey::Npc(_) => format!("tag=dw_npc_{safe},tag=dw_npc"),
        BodyKey::Actor(_) if staged => format!("tag=dw_pup_{safe}"),
        BodyKey::Actor(_) => format!("tag=dw_actor_{safe}"),
    }
}

/// [`body_terms`] as a one-body selector.
pub(super) fn body_sel(key: &BodyKey, staged: bool) -> String {
    format!("@e[{},limit=1]", body_terms(key, staged))
}

/// **Every distinct hop arc the campaign declares**, in the root walk's
/// order: an arc's place here, counted from 1, is the number its body carries
/// in [`HOP_ARC_OBJECTIVE`] — never 0, so a body with no entry there matches
/// no arc (`DW0495`).
pub(super) fn hop_arcs(c: &delvewright_dsl::Campaign) -> Vec<Vec<i32>> {
    let mut out: Vec<Vec<i32>> = Vec::new();
    for e in all_campaign_effects(c) {
        if let Verb::Hop { height } = &e.verb {
            let arc = hop_arc(*height);
            if !out.contains(&arc) {
                out.push(arc);
            }
        }
    }
    out
}

/// Whether any `hop` names this body.
fn body_hops(c: &delvewright_dsl::Campaign, walker: Walker, id: &str) -> bool {
    all_campaign_effects(c).into_iter().any(|e| {
        matches!(e.verb, Verb::Hop { .. })
            && e.bodies().iter().any(|k| {
                k.id() == id
                    && matches!(
                        (k, walker),
                        (BodyKey::Npc(_), Walker::Npc) | (BodyKey::Actor(_), Walker::Actor)
                    )
            })
    })
}

/// The line a walk's start function carries for a body that hops: the walk
/// takes the body, so a hop in flight ends (spec-0136 §3.1). Without it the
/// rest of the arc's steps would be added to the walk's last waypoint and
/// leave the body under the floor. `None` for a body no `hop` names.
pub(super) fn walk_ends_hop(
    c: &delvewright_dsl::Campaign,
    walker: Walker,
    id: &str,
) -> Option<String> {
    let safe = plan::safe_local(id);
    let terms = match walker {
        Walker::Npc => format!("tag=dw_npc_{safe},tag=dw_npc"),
        Walker::Actor => format!("tag=dw_pup_{safe}"),
    };
    body_hops(c, walker, id)
        .then(|| format!("scoreboard players reset @e[{terms}] {HOP_OBJECTIVE}"))
}

/// The latch of every walk the campaign plans for this body: `#mrun_<bare>` /
/// `#arun_<bare>`, set while that leg's driver runs.
pub(super) fn walk_latches(c: &delvewright_dsl::Campaign, key: &BodyKey) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in all_campaign_effects(c) {
        let latch = match (&e.verb, key) {
            (Verb::MoveNpc { npc, to, .. }, BodyKey::Npc(k)) if *npc == k.npc => format!(
                "#mrun_{}",
                movenpc_bare(npc.as_str(), to, &crate::compiler::nav::gate_key(e))
            ),
            (Verb::MoveActor { actor, to, .. }, BodyKey::Actor(k)) if *actor == k.actor => {
                format!(
                    "#arun_{}",
                    moveactor_bare(actor.as_str(), to, &crate::compiler::nav::gate_key(e))
                )
            }
            _ => continue,
        };
        if !out.contains(&latch) {
            out.push(latch);
        }
    }
    out
}

/// The objectives a hop needs, declared by `load`; empty for a campaign with
/// no `hop`, so its `load` is byte-identical.
pub(super) fn load_lines(c: &delvewright_dsl::Campaign) -> Vec<String> {
    if hop_arcs(c).is_empty() {
        return Vec::new();
    }
    vec![
        format!("scoreboard objectives add {HOP_OBJECTIVE} dummy"),
        format!("scoreboard objectives add {HOP_ARC_OBJECTIVE} dummy"),
    ]
}

/// The root `tick`'s line that steps every body in the air; empty for a
/// campaign with no `hop`, so its `tick` is byte-identical.
pub(super) fn tick_lines(plan: &Plan) -> Vec<String> {
    if hop_arcs(plan.campaign).is_empty() {
        return Vec::new();
    }
    vec![format!("function {}:{HOP_TICK_FN}", plan.namespace)]
}

/// The steps of one arc, one line per tick: on the tick the body has
/// `remaining` ticks left it moves by that tick's rise or fall, relative to
/// where it is. Each step is an exact binary fraction and the steps sum to
/// zero ([`hop_arc`]), so the last one puts the body on the height it left.
/// The body is `immovable` throughout: nothing here writes that field.
pub(super) fn hop_arc_lines(arc: &[i32]) -> Vec<String> {
    let n = arc.len();
    let mut out = Vec::new();
    let mut prev = 0;
    for (k, h) in arc.iter().enumerate() {
        let step = h - prev;
        prev = *h;
        if step != 0 {
            out.push(format!(
                "execute if score @s {HOP_OBJECTIVE} matches {} run tp @s ~ ~{} ~",
                n - k,
                hop_decimal(step)
            ));
        }
    }
    out
}

/// The hop driver (spec-0136 §3.1): `hop_tick`, called by the root `tick`,
/// which runs `hop_step` as each body in the air; one `hop_arc_<i>` per distinct arc; and one
/// `hop_start_<i>` a `hop` effect calls as the body. Empty for a campaign
/// with no `hop`.
pub(super) fn hop_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let arcs = hop_arcs(plan.campaign);
    if arcs.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut step: Vec<String> = Vec::new();
    for (i, arc) in arcs.iter().enumerate() {
        let i = i + 1;
        step.push(format!(
            "execute if score @s {HOP_ARC_OBJECTIVE} matches {i} run function {ns}:hop_arc_{i}"
        ));
        out.push((format!("hop_arc_{i}"), lines(&hop_arc_lines(arc))));
        out.push((
            format!("hop_start_{i}"),
            lines(&[
                format!("scoreboard players set @s {HOP_OBJECTIVE} {}", arc.len()),
                format!("scoreboard players set @s {HOP_ARC_OBJECTIVE} {i}"),
            ]),
        ));
    }
    step.push(format!("scoreboard players remove @s {HOP_OBJECTIVE} 1"));
    out.insert(0, (HOP_STEP_FN.to_string(), lines(&step)));
    out.insert(
        0,
        (
            HOP_TICK_FN.to_string(),
            lines(&[format!(
                "execute as @e[scores={{{HOP_OBJECTIVE}=1..}}] at @s run function {ns}:{HOP_STEP_FN}"
            )]),
        ),
    );
    out
}

/// Emit a staged body's move (spec-0136 §3), one body after another in the
/// order the effect lists them. A body the campaign does not declare emits
/// nothing and is `DW0112` long before here.
pub(super) fn emit_move(plan: &Plan, eff: &QuestEffect, body: &mut Vec<String>) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    for key in eff.bodies() {
        if key.resolve(c).is_none() {
            continue;
        }
        let staged = body_sel(&key, true);
        let any = body_sel(&key, false);
        let watches = crate::compiler::watching::body_watches(plan, key.id());
        match &eff.verb {
            Verb::Hop { height } => {
                let arc = hop_arc(*height);
                let i = hop_arcs(c)
                    .iter()
                    .position(|a| *a == arc)
                    .expect("every `hop` the campaign declares has its arc")
                    + 1;
                // A body a walk is carrying does not hop: the walk writes its
                // height every tick.
                let walking: String = walk_latches(c, &key)
                    .iter()
                    .map(|l| format!("unless score {l} dw.sys matches 1 "))
                    .collect();
                body.push(format!(
                    "execute {walking}as {staged} unless score @s {HOP_OBJECTIVE} matches 1.. run function {ns}:hop_start_{i}"
                ));
            }
            Verb::Pose { pose } => {
                body.push(format!(
                    "data merge entity {staged} {{pose:\"{}\"}}",
                    pose.nbt()
                ));
            }
            Verb::Turn { by } => {
                if watches {
                    body.push(crate::compiler::watching::yield_line(&body_terms(
                        &key, true,
                    )));
                }
                body.push(format!(
                    "execute as {staged} at @s run rotate @s ~{} ~",
                    num(*by)
                ));
            }
            Verb::Face { to } => match to {
                FaceTo::Yaw(yaw) => {
                    if watches {
                        body.push(crate::compiler::watching::yield_line(&body_terms(
                            &key, true,
                        )));
                    }
                    body.push(format!("execute as {staged} run rotate @s {} 0", num(*yaw)));
                }
                FaceTo::Mark(mark) => {
                    let Some(anchor) = anchor_point_any(plan, mark.anchor.as_str()) else {
                        continue; // unresolved anchor (`DW0360` owns it)
                    };
                    let cell = mark.cell(anchor);
                    if watches {
                        body.push(crate::compiler::watching::yield_line(&body_terms(
                            &key, true,
                        )));
                    }
                    body.push(format!(
                        "execute as {staged} at @s anchored eyes run rotate @s facing {} {} {}",
                        f64::from(cell[0]) + 0.5,
                        f64::from(cell[1]) + 0.5,
                        f64::from(cell[2]) + 0.5
                    ));
                }
                FaceTo::Watch(_) => {
                    if watches {
                        body.push(format!(
                            "tag @e[{}] add {}",
                            body_terms(&key, true),
                            crate::compiler::watching::WATCH_TAG
                        ));
                    }
                }
            },
            Verb::Hold { slot, item } => {
                let item = match item {
                    Some(i) if i.contains(':') => i.clone(),
                    Some(i) => format!("minecraft:{i}"),
                    None => "minecraft:air".to_string(),
                };
                body.push(format!(
                    "item replace entity {any} {} with {item}",
                    slot.item_slot()
                ));
            }
            Verb::Resize { scale } => {
                body.push(format!(
                    "execute as {any} run attribute @s minecraft:scale base set {}",
                    num(*scale)
                ));
            }
            _ => {}
        }
    }
}

/// **Every effect that keeps a cadence, with the function it is emitted
/// under** — `cad_<root>_<n>`, named by position exactly as a timeline is
/// ([`sequence_sites`]): `<root>` is the effect root's key and `<n>` the
/// effect's index among the root's cadenced effects in its deep walk.
///
/// Keyed by what the generated body depends on — the verb, the audience and
/// the cadence; the guard wraps the call — and the first declaration wins, so
/// two identical declarations share one function and one counter.
pub(super) fn cadence_sites(c: &delvewright_dsl::Campaign) -> Vec<(&QuestEffect, String)> {
    let mut out: Vec<(&QuestEffect, String)> = Vec::new();
    plan::for_each_effect_root(c, &mut |site, effs| {
        let root = fn_safe(site.key.strip_prefix("fx.").unwrap_or(&site.key));
        let mut here: Vec<&QuestEffect> = Vec::new();
        for e in effs {
            push_effect_deep(e, &mut here);
        }
        let mut n = 0usize;
        for e in here {
            if !e.has_cadence() || !e.verb.keeps_cadence() {
                continue;
            }
            if out.iter().any(|(x, _)| same_cadence(x, e)) {
                continue;
            }
            out.push((e, format!("cad_{root}_{n}")));
            n += 1;
        }
    });
    out
}

/// Whether two effects generate one cadence body.
fn same_cadence(a: &QuestEffect, b: &QuestEffect) -> bool {
    a.verb == b.verb
        && a.audience == b.audience
        && a.within == b.within
        && a.repeat == b.repeat
        && a.stagger_ticks == b.stagger_ticks
}

/// The start function of a cadenced effect.
///
/// # Panics
///
/// If the effect is not one the campaign declares with a cadence: emission
/// synthesizes no cadence, and a synthesized one would call a function
/// [`cadence_fns`] never generated.
pub(super) fn cadence_fn(plan: &Plan, eff: &QuestEffect) -> String {
    cadence_sites(plan.campaign)
        .into_iter()
        .find(|(e, _)| same_cadence(e, eff))
        .map(|(_, name)| name)
        .expect("every cadenced effect emission lowers is one the campaign declares")
}

/// `eff` with no cadence, addressed to `keys` alone.
fn narrowed(eff: &QuestEffect, keys: Option<&[BodyKey]>) -> QuestEffect {
    use delvewright_dsl::{EffectAudience, ParticleAt, SoundAt};
    let mut one = eff.clone();
    one.repeat = None;
    one.stagger_ticks = None;
    one.when = None;
    one.happening = None;
    let Some(keys) = keys else {
        return one;
    };
    if one.audience.as_ref().is_some_and(EffectAudience::is_staged) {
        one.audience = Some(EffectAudience::Bodies(keys.to_vec()));
        return one;
    }
    match &mut one.verb {
        Verb::Particle {
            at: ParticleAt::Bodies(at),
            ..
        } => at.bodies = keys.to_vec(),
        Verb::PlaySound {
            at: Some(SoundAt::Bodies { bodies }),
            ..
        } => *bodies = keys.to_vec(),
        _ => {}
    }
    one
}

/// The functions of every cadenced effect (spec-0136 §4).
///
/// `cad_<…>` starts it: each group of bodies — the whole cast, or with a
/// stagger each body in the order listed — is called at once or scheduled at
/// its delay. `cad_<…>_<g>` is one group's firing: the effect's own commands,
/// then, when it repeats, a count down and its own re-schedule `every_ticks`
/// later. `schedule` replaces a pending call of the same function, so firing
/// the effect again while it runs restarts it instead of doubling it.
///
/// Every body is emitted from the server command source: the scheduler has no
/// acting player, and the first firing must not differ from the second
/// (`DW1102` refuses `audience: actor` beside a cadence).
pub(super) fn cadence_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for (eff, base) in cadence_sites(plan.campaign) {
        let keys = eff.bodies();
        let stagger = eff.stagger_ticks.unwrap_or(0);
        let groups: Vec<Option<Vec<BodyKey>>> = if stagger > 0 && keys.len() > 1 {
            keys.iter().map(|k| Some(vec![k.clone()])).collect()
        } else {
            vec![None]
        };
        let (times, every) = eff.repeat.map_or((1, 0), |r| (r.times, r.every_ticks));
        let mut start: Vec<String> = Vec::new();
        for (g, group) in groups.iter().enumerate() {
            let name = format!("{base}_{g}");
            let counter = format!("#{name}");
            if times > 1 {
                start.push(format!("scoreboard players set {counter} dw.sys {times}"));
            }
            let delay = stagger * g as u32;
            if delay == 0 {
                start.push(format!("function {ns}:{name}"));
            } else {
                start.push(format!("schedule function {ns}:{name} {delay}t"));
            }
            let one = narrowed(eff, group.as_deref());
            let mut b: Vec<String> = Vec::new();
            emit_quest_effect(plan, &one, Audience::Scheduled, &mut b);
            if times > 1 {
                b.push(format!("scoreboard players remove {counter} dw.sys 1"));
                b.push(format!(
                    "execute if score {counter} dw.sys matches 1.. run schedule function {ns}:{name} {every}t"
                ));
            }
            out.push((name, lines(&b)));
        }
        out.push((base, lines(&start)));
    }
    out
}

/// The `particle` commands of one at staged bodies (spec-0136 §3.6): the
/// effect's audience sees it at each body, `up` blocks above its feet.
pub(super) fn particle_at_bodies(
    plan: &Plan,
    at: &delvewright_dsl::ParticleAtBodies,
    id: &str,
    tail: &str,
    who: &str,
    body: &mut Vec<String>,
) {
    for key in &at.bodies {
        if key.resolve(plan.campaign).is_none() {
            continue;
        }
        body.push(format!(
            "execute at {} run particle {id} ~ ~{} ~ {tail} {who}",
            body_sel(key, false),
            if at.up == 0.0 {
                String::new()
            } else {
                num(at.up)
            }
        ));
    }
}
