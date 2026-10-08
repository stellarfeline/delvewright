//! NPCs: summon, spawn, despawn, and move-npc walkers.

use super::*;

/// Whether a stage-2 NPC declares `deferred: true` (DSL v0.6) — it is not summoned
/// at world init and enters only via a `spawn-npc` effect.
pub(super) fn npc_is_deferred(c: &delvewright_dsl::Campaign, npc_id: &str) -> bool {
    c.npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)
        .map(|n| n.deferred)
        .unwrap_or(false)
}

/// The one authority for an NPC's world presence: the `/summon` commands that place
/// its body (villager re-dress or mannequin) **and** its co-located interaction
/// hitbox at its declared anchor, with its name display.
///
/// Called from exactly two places — the world-init `setup_finish` block (a normal
/// NPC) and the generated `spawn_npc_<id>` function (a `deferred` NPC, DSL v0.6) —
/// so a scripted entrance produces byte-for-byte the same entity as an init-time
/// one. Extracted for that duality; the command text is unchanged from pre-0.6, so
/// a campaign with no deferred NPC is byte-identical.
pub(super) fn npc_summon_commands(
    c: &delvewright_dsl::Campaign,
    plan: &Plan,
    npc: &plan::NpcPlan,
) -> Vec<String> {
    let area = plan.npc_area(&npc.npc_id).unwrap_or("");
    let dsl_npc = c
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc.npc_id);
    let anchor = dsl_npc.map(|n| n.anchor.as_str()).unwrap_or("");
    // **The one authority answers this**, as `BodyScope::Declared` — the same
    // function the cast ledger's per-beat station and `DW0461` ask. A private
    // `(area, name)` lookup here is how the world-init summon and the plan came
    // to describe two buildings 256 blocks apart in one build.
    let station = plan::body_station(&plan.anchors, plan::BodyScope::Declared { area }, anchor);
    let offset = dsl_npc.map(|n| n.offset).unwrap_or([0, 0, 0]);
    let (pos, facing) = match &station {
        plan::BodyStation::At {
            anchor: ResolvedAnchor::Point { pos, facing },
            ..
        } => (
            delvewright_dsl::offset_cell(*pos, offset),
            facing.as_deref(),
        ),
        _ => ([0, plan::BASE_Y, 0], None),
    };
    let name = dsl_npc.map(|n| n.name.as_str()).unwrap_or("NPC");
    let base = dsl_npc
        .map(|n| n.base_entity.as_str())
        .unwrap_or("minecraft:villager");
    let yaw = facing_yaw(facing);
    let p = ent_xyz(pos);
    let mut out = Vec::new();
    if let Some(skin) = dsl_npc.and_then(|n| n.skin.as_ref()) {
        // DSL v0.4 mannequin NPC (spec-0008 §6 / spec-0009). The label is
        // emitted as `description`, a **text-component SNBT compound**
        // (`{text:"…"}`) — NOT a stringified-JSON text component
        // (`'{"text":…}'`), which renders as literal raw JSON above the head on
        // 1.21.11 (owner-verified). NoAI/PersistenceRequired/VillagerData are
        // dropped (silently ignored on a mannequin); the interaction hitbox is
        // unchanged.
        // `pose:"standing"` is emitted explicitly: a mannequin summoned without
        // it serializes its pose as `DYING` (a gametest save-teardown warning),
        // wrong data for a standing NPC. Valid 1.21.11 mannequin poses: standing,
        // crouching, swimming, fall_flying, sleeping (spec-0009 template).
        out.push(format!(
            "summon minecraft:mannequin {} {} {} {{profile:{{texture:\"delvewright:npc/{}\",model:\"{}\"}},immovable:1b,pose:\"standing\",Invulnerable:1b,Silent:1b,Rotation:[{yaw}f,0f],description:{},Tags:[\"dw_npc\",\"{}\"]}}",
            p[0], p[1], p[2], skin.texture_id, skin.model.token(),
            snbt_text_component(name), npc.tag
        ));
    } else {
        // CustomName is a 1.21.11 text component, emitted as a plain SNBT string
        // (renders correctly, incl. death messages).
        let cname_field = snbt_component(name);
        let pose = mannequin_pose_nbt(base);
        out.push(format!(
            "summon {base} {} {} {} {{NoAI:1b,Invulnerable:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b{pose},Rotation:[{yaw}f,0f],Tags:[\"dw_npc\",\"{}\"],CustomName:{},CustomNameVisible:1b,VillagerData:{{profession:\"minecraft:none\",type:\"minecraft:plains\",level:1}}}}",
            p[0], p[1], p[2], npc.tag, cname_field
        ));
    }
    // The interaction hitbox also carries the tag of every left-click trigger
    // that watches this NPC — see `npc_hitbox_trigger_tags`.
    let mut tags = vec![npc.tag.clone()];
    // A `strike` at the anchor rides this hitbox only while the body stands on
    // the anchor's own cell (spec-0066).
    let stand_anchor = if offset == [0, 0, 0] { anchor } else { "" };
    tags.extend(npc_hitbox_trigger_tags(c, stand_anchor, &npc.npc_id));
    let tag_list = tags
        .iter()
        .map(|t| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(",");
    out.push(format!(
        "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{BORNE_NBT}{tag_list}]}}",
        p[0], p[1], p[2]
    ));
    out
}

/// The `dw_trig_<id>` tags of every `strike` trigger whose `at` anchor is
/// `anchor`, in campaign declaration order (deterministic).
///
/// **Why an NPC's hitbox wears a trigger's tag.** A `strike` trigger is detected
/// by reading the `attack` record off a `minecraft:interaction` entity — the
/// vanilla primitive for "a player left-clicked this". When the trigger's anchor
/// is also where an NPC stands, the NPC's hitbox is the entity a click actually
/// reaches, and the NPC's body is `Invulnerable`, so a trigger listening on an
/// entity of its own could simply never fire (round-4 island QA:
/// `wake-the-giant` on the sleeping giant's anchor was dead).
///
/// The NPC hitbox is the trigger's **sole** carrier: `env_trigger_setup`
/// suppresses the trigger's own summon for this collision. Round-4 shared the
/// tag but kept both entities; the two exactly co-located hitboxes then made
/// the *right*-click pick ambiguous, and when the standalone won, the dialogue
/// advancement (keyed on `Tags:["dw_npc_<n>"]`) never fired — the round-6
/// island soft-lock (Polyphemus untalkable after the boulder seal). One cell,
/// one hitbox ends both failure modes. Empty for an anchor with no co-located
/// strike trigger, so every campaign without this collision stays
/// byte-identical.
///
/// Scope: `strike` only. Right-click (`use`) on an NPC already belongs to the
/// dialogue advancement, so a co-located `use` trigger is an authoring
/// conflict, rejected at validate time (`DW0350`).
/// The first `(trigger, npc id, npc entity tag)` triple whose trigger rides an
/// NPC's own interaction hitbox — either a `strike-npc` naming that NPC (DSL
/// v0.6) or a `strike` whose anchor is that NPC's stand anchor. The collision
/// [`npc_hitbox_trigger_tags`] resolves. Campaign order (deterministic); `None`
/// when no trigger rides an NPC.
pub(super) fn first_strike_trigger_on_npc<'a>(
    plan: &'a Plan,
) -> Option<(&'a delvewright_dsl::EnvTrigger, String, String)> {
    plan.campaign
        .quests
        .content
        .triggers
        .iter()
        .find_map(|t| npc_ridden_by(plan, t).map(|n| (t, n.npc_id.clone(), n.tag.clone())))
}

/// The planned NPC whose own interaction hitbox carries the left-click trigger
/// `t` — the one rule, read by the emitter (which then summons no standalone
/// hitbox) and by the visibility proof (`DW0963`: the NPC's body is the visible
/// thing the strike sits on). `None` when `t` rides no NPC.
pub(crate) fn npc_ridden_by<'a>(
    plan: &'a Plan,
    t: &delvewright_dsl::EnvTrigger,
) -> Option<&'a crate::compiler::plan::NpcPlan> {
    let c = plan.campaign;
    plan.npcs.iter().find(|n| {
        let decl = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|d| d.id.as_str() == n.npc_id);
        // A body at an offset does not stand on its anchor's cell, so a
        // `strike` at that anchor is not on its hitbox (spec-0066).
        let anchor = decl
            .filter(|d| d.offset == [0, 0, 0])
            .map(|d| d.anchor.as_str())
            .unwrap_or("");
        trigger_rides_npc(t, anchor, &n.npc_id)
    })
}

/// Whether `t` is a left-click trigger carried by the interaction hitbox of the
/// NPC `npc_id` standing at `anchor`.
///
/// Two spellings, one mechanism. `strike-npc` (DSL v0.6) names the NPC
/// **directly** and is the intended form: it works wherever the NPC stands and
/// whatever its body is, because it never asks for a cell. A bare `strike` whose
/// `at` happens to be the NPC's own anchor is the pre-0.6 spelling of the same
/// thing, kept working: co-locating a second interaction entity with an NPC is
/// the one-cell-two-hitboxes defect (`DW0350`/`DW0359`), so the compiler shares
/// the NPC's hitbox instead of summoning one.
pub(super) fn trigger_rides_npc(
    t: &delvewright_dsl::EnvTrigger,
    anchor: &str,
    npc_id: &str,
) -> bool {
    use delvewright_dsl::TriggerOn;
    match &t.on {
        TriggerOn::StrikeNpc { npc } => npc.as_str() == npc_id,
        TriggerOn::Strike => !anchor.is_empty() && t.at_anchor() == Some(anchor),
        _ => false,
    }
}

/// The `dw_trig_<id>` tags every left-click trigger riding this NPC's hitbox
/// contributes, in campaign declaration order (deterministic). Empty for an NPC
/// no trigger watches, so every campaign without one stays byte-identical.
pub(super) fn npc_hitbox_trigger_tags(
    c: &delvewright_dsl::Campaign,
    anchor: &str,
    npc_id: &str,
) -> Vec<String> {
    c.quests
        .content
        .triggers
        .iter()
        .filter(|t| trigger_rides_npc(t, anchor, npc_id))
        .map(|t| format!("dw_trig_{}", plan::safe_local(t.id.as_str())))
        .collect()
}

/// The generated function name for a `spawn-npc` effect (DSL v0.6).
pub(super) fn spawn_npc_fn(npc: &str) -> String {
    format!("spawn_npc_{}", plan::safe_local(npc))
}

/// Every NPC a compiled `spawn-npc` site names — the quest/trigger/trap effect
/// trees ([`all_campaign_effects`], nesting included) plus every dialogue option's
/// `spawn-npc`, which the option handler compiles the very same call for.
///
/// This is the emitted-call set for [`spawn_npc_fns`], so the two agree by
/// construction rather than by convention (`DW0497`).
pub(super) fn spawn_npc_sites(c: &delvewright_dsl::Campaign) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for e in all_campaign_effects(c) {
        if let Some(npc) = e.spawn_npc() {
            out.insert(npc.as_str().to_string());
        }
    }
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for opt in &node.options {
                for eff in &opt.effects {
                    if let Some(npc) = eff.spawn_npc() {
                        out.insert(npc.as_str().to_string());
                    }
                }
            }
        }
    }
    out
}

/// The generated function name for a `despawn-npc` effect.
pub(super) fn despawn_npc_fn(npc: &str) -> String {
    format!("despawn_npc_{}", plan::safe_local(npc))
}

/// `despawn_npc_<id>` functions: one per NPC any `despawn-npc` effect removes,
/// taken from the same effect walk ([`all_campaign_effects`]) that compiles the
/// calls, so each call has its callee by construction (`DW0497`).
/// The body and its interaction hitbox both carry the per-NPC id tag
/// (spec-0008 §5) and leave together through [`removal_lines`] as
/// [`Exit::Unseen`]: an NPC the story sends away is never seen to die.
pub(super) fn despawn_npc_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let sites: BTreeSet<String> = all_campaign_effects(plan.campaign)
        .into_iter()
        .filter_map(|e| e.despawn_npc())
        .map(|npc| npc.as_str().to_string())
        .collect();
    sites
        .into_iter()
        .map(|npc| {
            let tag = format!("dw_npc_{}", plan::safe_local(&npc));
            (
                despawn_npc_fn(&npc),
                lines(&removal_lines(ns, &tag, false, Exit::Unseen)),
            )
        })
        .collect()
}

/// `spawn_npc_<id>` functions (DSL v0.6): one per NPC any `spawn-npc` effect
/// summons, the scripted-entrance dual of `despawn-npc`. A campaign that fires
/// none and defers none emits nothing here, so it is byte-identical to pre-0.6.
///
/// **Not "one per deferred NPC".** `DW0197` guarantees every `deferred` NPC has a
/// spawn site, so the deferred set is contained in the call set — but the converse
/// was never true: `spawn-npc` on a NON-deferred NPC is legal content (it is how a
/// character comes back after a `despawn-npc`), and it compiled a
/// `function <ns>:spawn_npc_<id>` call against a function nobody emitted. The call
/// loaded fine and did nothing, so the character stayed gone. That is the island's
/// wave defect in a second emitter, and it is why the registration walk is now the
/// call walk ([`spawn_npc_sites`]) rather than a parallel property scan. The
/// deferred set is unioned in so ordering and output for existing campaigns are
/// untouched.
///
/// Each of the two summons is **independently** idempotent, so a re-fired
/// `spawn-npc` never doubles an entity — and so an entrance fired for an NPC
/// already standing at its mark is exactly the no-op it reads as. Body and hitbox
/// share the per-NPC id tag, so the guards discriminate on the body-only `dw_npc`
/// tag: the body is guarded by `[tag=dw_npc,tag=<id>]`, the hitbox by its negation
/// `[tag=<id>,tag=!dw_npc]` — a single `unless entity @e[tag=<id>]` guard on both
/// lines would let the body's own summon suppress the hitbox.
pub(super) fn spawn_npc_fns(plan: &Plan) -> Vec<(String, String)> {
    let c = plan.campaign;
    let sites = spawn_npc_sites(c);
    let mut out = Vec::new();
    for npc in &plan.npcs {
        if !npc_is_deferred(c, &npc.npc_id) && !sites.contains(npc.npc_id.as_str()) {
            continue;
        }
        let cmds = npc_summon_commands(c, plan, npc);
        let body: Vec<String> = cmds
            .iter()
            .map(|cmd| {
                let guard = if cmd.starts_with("summon minecraft:interaction ") {
                    format!("@e[tag={},tag=!dw_npc]", npc.tag)
                } else {
                    format!("@e[tag=dw_npc,tag={}]", npc.tag)
                };
                format!("execute unless entity {guard} run {cmd}")
            })
            .collect();
        out.push((spawn_npc_fn(&npc.npc_id), lines(&body)));
    }
    out
}

/// The generated function name for a `move-npc` effect (content-derived key, so
/// the start-caller and the generator agree without threading an index).
pub(super) fn movenpc_fn(npc: &str, to: &delvewright_dsl::Mark, gate_key: &str) -> String {
    format!("mv_{}_{}{gate_key}", plan::safe_local(npc), mark_key(to))
}

/// The function-name component a destination mark contributes: the anchor's
/// local name, and for a non-zero offset `_o<x>_<y>_<z>` with a negative
/// component spelled `m<n>` (spec-0066). A zero offset adds nothing, so a walk to
/// a bare anchor keeps the name it has always had.
pub(super) fn mark_key(to: &delvewright_dsl::Mark) -> String {
    let base = plan::safe_local(to.anchor.as_str());
    if !to.is_offset() {
        return base;
    }
    let c = |v: i32| {
        if v < 0 {
            format!("m{}", -i64::from(v))
        } else {
            v.to_string()
        }
    };
    format!(
        "{base}_o{}_{}_{}",
        c(to.offset[0]),
        c(to.offset[1]),
        c(to.offset[2])
    )
}

/// Which kind of body a walk driver moves: an NPC (`move-npc`, `mv_tick_*`) or an
/// actor's puppet (`move-actor`, `ma_tick_*`). Only the names of the scores a
/// driver's supersession guard reads differ between the two.
#[derive(Clone, Copy)]
pub(super) enum Walker {
    Npc,
    Actor,
}

/// The two scores a supersedable walk driver's staleness guard compares: the
/// driver's own stamp and its body's current walk generation (`#mown_<bare>` /
/// `#mgen_<npc>` for an NPC, `#aown_<bare>` / `#agen_<actor>` for a puppet). The
/// driver is stale exactly when `own < gen`. The start function, the guard and a
/// template that invokes the driver directly ([`walk_claim`]) all name them here.
pub(super) fn walk_stamp(walker: Walker, bare: &str, safe: &str) -> (String, String) {
    let (own, gen_) = match walker {
        Walker::Npc => ("mown", "mgen"),
        Walker::Actor => ("aown", "agen"),
    };
    (format!("#{own}_{bare}"), format!("#{gen_}_{safe}"))
}

/// Every body that owns two or more planned walks. Such a body's drivers carry the
/// supersession guard; a body with one walk can never be superseded and carries
/// none of it.
pub(super) fn supersedable_walkers<'a>(
    bodies: impl IntoIterator<Item = &'a str>,
) -> BTreeSet<&'a str> {
    let mut legs: BTreeMap<&str, usize> = BTreeMap::new();
    for b in bodies {
        *legs.entry(b).or_insert(0) += 1;
    }
    legs.into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(b, _)| b)
        .collect()
}

/// What a PackTest template runs before it invokes a walk driver directly: the
/// driver takes its body's current generation as its own stamp, so its guard
/// reads it as the live walk whatever walks earlier templates fired. The
/// scoreboard is global to the whole suite, so a sibling that fired two walks for
/// the same body leaves this driver's stamp behind the generation, and a driver
/// invoked without the claim returns before it teleports. The generation is taken,
/// not bumped: a walk a sibling is running is not superseded by this template.
/// Empty for a body whose driver carries no guard.
pub(super) fn walk_claim(
    walker: Walker,
    bare: &str,
    safe: &str,
    supersedable: bool,
) -> Vec<String> {
    if !supersedable {
        return Vec::new();
    }
    let (own, gen_) = walk_stamp(walker, bare, safe);
    vec![format!(
        "scoreboard players operation {own} dw.sys = {gen_} dw.sys"
    )]
}

/// The scoreboard-safe suffix shared by a move's driver functions/sentinels.
pub(super) fn movenpc_bare(npc: &str, to: &delvewright_dsl::Mark, gate_key: &str) -> String {
    movenpc_fn(npc, to, gate_key)
        .strip_prefix("mv_")
        .unwrap_or("move")
        .to_string()
}

/// `move-npc` functions (spec-0008 addendum): a **collision-safe walked path**,
/// not a single teleport. The path is planned by A* over the solved voxel grid
/// (`crate::compiler::nav`) at compile time; here we emit a self-scheduling per-tick driver
/// that teleports the NPC body + interaction hitbox (both carry the id tag) along
/// the waypoint polyline at the planned speed. Client interpolation smooths the
/// per-tick jumps into a walk (spike-verified). Deduped by content key; empty for
/// a campaign with no moves.
///
/// Each `tp` carries the **planned yaw** for that tick (`nav::yaws_along`, pitch
/// always 0 — a walk is level by construction). A rotation-less `tp` leaves the
/// body's yaw at whatever its summon or previous beat set, so an NPC routed the
/// other way slides backwards for the whole walk. Actor puppets carry their
/// tangent yaw, and `move-npc` holds to the same standard.
///
/// An `on_arrive` bundle (DSL v0.6, parity with `move-actor`) fires on the
/// driver's **final-waypoint tick** — exactly the arrival detection `ma_tick`
/// uses — via a generated `mv_arrive_<key>` function. A bare `move-npc` emits no
/// arrive hook and stays byte-identical to pre-0.6 output.
///
/// # Supersession — one body, one live driver
///
/// A driver's re-entry latch `#mrun_<bare>` is keyed per **(npc, to, gate)**:
/// it stops a walk from restarting *itself* and knows nothing about the body's other
/// walks. So a second `move-npc` fired at the same NPC while an earlier walk was
/// still running used to leave **two** drivers alive, both teleporting the same
/// entity every tick; the interleave garbled the path and whichever walk had more
/// remaining ticks wrote the final position — the body parked at the FIRST walk's
/// endpoint, not the last-fired one (root-caused live on the island, 2026-08-06: a
/// 408-tick beach→mouth walk overlapped by a 21-tick walk to checkpoint-1 left the
/// NPC 3.0 blocks off its cast-ledger cell, exactly on the harness's affordance
/// radius).
///
/// The contract is now **last fired wins**, carried by a per-NPC *walk generation*
/// score `#mgen_<npc>`: starting a walk bumps the generation and stamps it onto that
/// driver's `#mown_<bare>`; every driver tick first checks its own stamp is still the
/// current generation and, if not, drops its latch and returns without teleporting.
/// The superseded driver dies on its next scheduled tick, the new walk's tp sequence
/// runs alone from its own first waypoint (waypoints are precomputed from the walk's
/// declared start, so the new leg snaps to that first waypoint — the same instant
/// snap single-walk content already gets when a walk fires while its NPC stands
/// elsewhere). The staleness test is written as the positive `if own < gen`, never as
/// `unless own = gen`: with both scores unset a score comparison is *false*, and the
/// `unless` spelling would read that as "stale" and cancel a walk nothing superseded.
/// A PackTest that invokes a guarded driver directly (`v04_move`) cannot rely on the
/// scores being unset — the scoreboard is shared by the whole suite, and a sibling
/// that fired two walks for the body leaves this driver's stamp behind — so it claims
/// the driver first ([`walk_claim`]).
///
/// A body with only one planned walk can never be superseded, so it carries none of
/// this: campaigns whose NPCs each walk at most once stay byte-identical (ADR-0006).
pub(super) fn movenpc_fns(
    plan: &Plan,
    moves: &[crate::compiler::nav::MovePlan],
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    // A body with two or more walks can be caught mid-flight by a later one ⇒ its
    // drivers carry the generation guard.
    let guarded = supersedable_walkers(moves.iter().map(|m| m.npc.as_str()));
    for m in moves {
        let start_name = movenpc_fn(&m.npc, &m.to, &m.gate_key);
        let bare = movenpc_bare(&m.npc, &m.to, &m.gate_key);
        let safe = plan::safe_local(&m.npc);
        let total = m.ticks();
        let supersedable = guarded.contains(m.npc.as_str());
        // `#mown_<bare> < #mgen_<npc>` ⇔ a later walk for this body has started.
        let (own, gen_) = walk_stamp(Walker::Npc, &bare, &safe);
        let stale = format!("score {own} dw.sys < {gen_} dw.sys");
        // The on_arrive bundle for this (npc, to) — the first-seen effect,
        // matching the planner's dedup order (mirrors `actor_fns`).
        let on_arrive: &[QuestEffect] = all_campaign_effects(plan.campaign)
            .into_iter()
            .find_map(|e| match &e.verb {
                Verb::MoveNpc {
                    npc, to, on_arrive, ..
                } if npc.as_str() == m.npc && *to == m.to => Some(on_arrive.as_slice()),
                _ => None,
            })
            .unwrap_or(&[]);

        // start: guard re-entry, take the walk generation, reset the tick counter,
        // schedule the driver. The re-entry refusal is generation-aware: a latch left
        // armed by a driver this body has already superseded must not block the
        // re-fire of that same leg (it is itself a later walk, and wins).
        let mut start = Vec::new();
        if supersedable {
            start.push(format!(
                "execute if score #mrun_{bare} dw.sys matches 1 unless {stale} run return fail"
            ));
            start.push(format!("scoreboard players add {gen_} dw.sys 1"));
            start.push(format!(
                "scoreboard players operation {own} dw.sys = {gen_} dw.sys"
            ));
        } else {
            start.push(format!(
                "execute if score #mrun_{bare} dw.sys matches 1 run return fail"
            ));
        }
        start.push(format!("scoreboard players set #mrun_{bare} dw.sys 1"));
        start.push(format!("scoreboard players set #mt_{bare} dw.sys 0"));
        start.push(format!("schedule function {ns}:mv_tick_{bare} 1t"));
        out.push((start_name, lines(&start)));

        // per-tick driver: tp both body + hitbox to waypoint[t], advance, and
        // reschedule until the path is walked; the final waypoint is the target.
        let mut tick: Vec<String> = Vec::new();
        if supersedable {
            // Superseded: drop the latch (so this leg can be fired again later) and
            // stop — no teleport, no arrive hook, no reschedule. The `schedule` this
            // driver queued before it lost the body is what brought us here; not
            // rescheduling is what ends it.
            tick.push(format!(
                "execute if {stale} run scoreboard players set #mrun_{bare} dw.sys 0"
            ));
            tick.push(format!("execute if {stale} run return fail"));
        }
        for (t, (w, y)) in m.waypoints.iter().zip(m.yaws.iter()).enumerate() {
            tick.push(format!(
                "execute if score #mt_{bare} dw.sys matches {t} run tp @e[tag=dw_npc_{safe}] {} {} {} {y} 0",
                fmt_f64(w[0]),
                fmt_f64(w[1]),
                fmt_f64(w[2])
            ));
        }
        if !on_arrive.is_empty() {
            tick.push(format!(
                "execute if score #mt_{bare} dw.sys matches {total} run function {ns}:mv_arrive_{bare}"
            ));
        }
        tick.push(format!("scoreboard players add #mt_{bare} dw.sys 1"));
        tick.push(format!(
            "execute if score #mt_{bare} dw.sys matches {}.. run scoreboard players set #mrun_{bare} dw.sys 0",
            total + 1
        ));
        tick.push(format!(
            "execute unless score #mt_{bare} dw.sys matches {}.. run schedule function {ns}:mv_tick_{bare} 1t",
            total + 1
        ));
        out.push((format!("mv_tick_{bare}"), lines(&tick)));

        if !on_arrive.is_empty() {
            // Server command source: the driver that calls this reached us from
            // `schedule`, so there is no `@s` (see `Audience`).
            let arrive = emit_effect_bundle(plan, on_arrive, Audience::Scheduled);
            out.push((format!("mv_arrive_{bare}"), lines(&arrive)));
        }
    }
    out
}
