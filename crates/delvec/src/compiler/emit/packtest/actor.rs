use super::*;

/// spec-0021 actor-equipment PackTest: an equipped actor's puppet spawns wearing
/// its gear, and the unleashed twin still wears it.
///
/// The handoff is the part worth proving on a real server: `unleash` kills the
/// puppet and summons a fresh entity, so gear that rode only on the puppet would
/// vanish the instant the elite came alive — a regression invisible to any
/// compile-time check. Emitted only for a campaign with an equipped actor.
///
/// **A skinned body is not excluded, and excluding it was the defect.** The
/// filter used to demand `skin.is_none()`, which was true of the engine it was
/// written against — the skin branch of the puppet summon dropped `equipment`
/// outright, so a skinned actor had no gear for this to find. That is fixed, and
/// the exclusion then read exactly backwards: it refused to look at the one case
/// that had ever been broken, and it was an opt-out the defect itself could
/// supply. It was measured doing so — a campaign whose every actor is skinned
/// emitted no actor-equipment test at all and its proof set shrank by one with
/// nothing saying why. The body a campaign dresses is now irrelevant to whether
/// its gear is proved.
pub(super) fn emit_actor_equipment_packtest(plan: &Plan, out: &mut BuildOutput) {
    // One test per BODY KIND among the equipped actors, taking the first actor
    // of each kind. What is under test is whether the body a campaign dresses
    // changes whether its gear survives, so the kinds are the population and a
    // second guard in the same livery would add a run and prove nothing. A
    // campaign that dresses both a plain entity and a mannequin gets both.
    let mut seen: Vec<String> = Vec::new();
    for a in plan
        .campaign
        .quests
        .content
        .actors
        .iter()
        .filter(|a| a.equipment.is_some())
    {
        let body = crate::compiler::nav::actor_body_entity(a);
        if seen.contains(&body) {
            continue;
        }
        seen.push(body.clone());
        emit_one_actor_equipment_packtest(plan, a, &body, out);
    }
}

/// One body kind's gear test. Split out so the population above is a plain loop
/// over the kinds rather than a loop with a body inlined in it.
fn emit_one_actor_equipment_packtest(
    plan: &Plan,
    a: &delvewright_dsl::Actor,
    body: &str,
    out: &mut BuildOutput,
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    // Every slot the actor fills is asserted on both bodies: the live proof that
    // the pinned server stores each key the summon writes (spec-0067 §3 —
    // `body` and `saddle` included).
    let eq = a.equipment.as_ref().expect("filtered on Some");
    let filled: Vec<(&str, &EquipItem)> = eq
        .pieces()
        .into_iter()
        .filter_map(|(slot, p)| p.map(|p| (slot.nbt(), p)))
        .collect();
    if filled.is_empty() {
        return;
    }
    let safe = plan::safe_local(a.id.as_str());
    let mut b = packtest_header(&format!(
        "{title}: actor `{}`, a {body}, keeps its gear across unleash (spec-0021)",
        a.id
    ));
    b.push(format!("function {ns}:setup"));
    // Clean slate: the shared batch server may already carry this actor.
    b.push(format!("kill @e[tag=dw_actor_{safe}]"));
    b.push(format!("function {ns}:spawn_actor_{safe}"));
    for (slot, piece) in &filled {
        b.push(format!(
            "execute store success score #aeqp dw.sys if data entity @e[tag=dw_pup_{safe},limit=1] equipment.{slot}{{id:\"{}\"}}",
            piece.item()
        ));
        b.push("assert score #aeqp dw.sys matches 1".to_string());
    }
    b.push(format!("function {ns}:unleash_{safe}"));
    // The twin is the actor-tagged entity that is NOT the puppet.
    for (slot, piece) in &filled {
        b.push(format!(
            "execute store success score #aeqt dw.sys if data entity @e[tag=dw_actor_{safe},tag=!dw_pup_{safe},limit=1] equipment.{slot}{{id:\"{}\"}}",
            piece.item()
        ));
        b.push("assert score #aeqt dw.sys matches 1".to_string());
    }
    b.push(format!("kill @e[tag=dw_actor_{safe}]"));
    // The body is in the name, so a campaign that dresses two kinds gets two
    // files rather than one overwriting the other. The namespace colon is
    // replaced rather than dropped: two ids differing only in namespace are two
    // kinds, and a resource location's path does not admit a colon.
    let body_local = body.replace([':', '-', '/', '.'], "_");
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_actor_equipment_{body_local}.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// v0.6 PackTests (spec-0014): a `spawn-actor` puppet appears and both despawn
/// styles remove it; a `move-actor` walks its puppet to the destination cell (its
/// `on_arrive` bundle runs on the same final tick); `unleash-actor` swaps the NoAI
/// puppet for a real-AI twin. Single-tick assertable; sequence-exact-tick timing and
/// per-tick yaw/NBT are covered by compiler unit tests (they assert the emitted
/// commands directly — stronger and faster than a timing gametest). Emits nothing
/// when the campaign declares no actors.
/// The claim the three actor loops make, judged against the shipped bytes by
/// `DW0811`. Written from `quests.content.actors` — the authored list — so an
/// emitter that reverts to `actors.first()` still declares every actor here and
/// the refusal names the ones it stopped driving.
///
/// Both families in one claim because both loops walk the same list: an actor
/// with a spawn body and no unleash body is not a breach (`check_claims` judges
/// only bodies that exist), but an actor with either body and no template is.
pub(super) fn actor_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "actor",
        families: vec!["spawn_actor_".to_string(), "unleash_".to_string()],
        declared: plan
            .campaign
            .quests
            .content
            .actors
            .iter()
            .map(|a| plan::safe_local(a.id.as_str()))
            .collect(),
    }
}

pub(super) fn emit_v06_actor_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let actors = &c.quests.content.actors;
    if actors.is_empty() {
        return;
    }
    let mut write = |name: &str, body: Vec<String>| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&body).into_bytes(),
        );
    };

    // The four actor tests all drive the SAME first actor through its real (and
    // therefore shared) entity tags — `spawn_actor_<id>`'s idempotence guard is
    // `unless entity @e[tag=dw_actor_<id>]`, a tag the unleashed twin also
    // carries. On the shared-batch server a sibling's leftover (e.g. the twin
    // `v06_unleash` produced) therefore no-ops a later test's spawn while
    // matching none of its puppet asserts (the round-6 island flake:
    // `v06_spawn_idempotent` counted 0 puppets). Every actor test must
    // establish its own world: clear the actor tag on entry (never assume a
    // fresh world) and clear it again on exit (leave no poison for a sibling).
    // Each template is a single atomic function, so within it the entity state
    // cannot be interleaved.

    // spawn-actor + despawn kill/vanish: the puppet appears, and either style
    // removes it. The visible difference (kill = in-place death animation, vanish =
    // silent relocate-then-kill out of view) is a client-eyes distinction; CI
    // asserts both leave zero entities under the actor tag.
    // EVERY declared actor, not the first. `spawn_actor_<id>` and `unleash_<id>`
    // are per-actor bodies: each summons its OWN entity type, with its own NBT,
    // at its own cell — a bat at one anchor and a spider at another are not two
    // instances of one proof, they are two summons that can fail independently,
    // and a 1.21.11 function with one bad line is refused in its entirety. The
    // gallery shipped four actors and drove one of them here; three carried no
    // runtime proof from this file at all. Registered as a claim
    // (`actor_watch_claim`) so the walk cannot quietly stop at `first()` again.
    for a in actors {
        let safe = plan::safe_local(a.id.as_str());
        // Every scratch score is suffixed with the actor's own id. The suite is
        // ONE batch on ONE server with no ordering guarantee between templates,
        // so a score shared across sibling actors would be written by one
        // template and asserted by another (`crate::compiler::batchstate`).
        let mut b = packtest_header(&format!(
            "{}: spawn-actor `{}` appears; despawn kill & vanish both remove it",
            artifact_title(c),
            a.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!(
            "execute store result score #sp_sdsp_{safe} dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push(format!("assert score #sp_sdsp_{safe} dw.sys matches 1.."));
        // kill style removes it.
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!(
            "execute store result score #k_sdsp_{safe} dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push(format!("assert score #k_sdsp_{safe} dw.sys matches 0"));
        // re-spawn (idempotent), then vanish style also removes it — which also
        // leaves the world actor-free for the next test.
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!("tp @e[tag=dw_actor_{safe}] ~ -128 ~"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!(
            "execute store result score #v_sdsp_{safe} dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push(format!("assert score #v_sdsp_{safe} dw.sys matches 0"));
        write(&format!("v06_spawn_despawn_{safe}"), b);
    }

    // spawn-actor is idempotent (re-caging after unleash): two spawns yield exactly
    // one puppet, not two. Per actor for the same reason — the idempotence guard
    // is `unless entity @e[tag=dw_actor_<id>]`, a per-actor tag.
    for a in actors {
        let safe = plan::safe_local(a.id.as_str());
        let mut b = packtest_header(&format!(
            "{}: spawn-actor `{}` is idempotent (one puppet, not two)",
            artifact_title(c),
            a.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!(
            "execute store result score #n_sidm_{safe} dw.sys if entity @e[tag=dw_pup_{safe}]"
        ));
        b.push(format!("assert score #n_sidm_{safe} dw.sys matches 1"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        write(&format!("v06_spawn_idempotent_{safe}"), b);
    }

    // unleash-actor: the NoAI puppet (dw_pup) is replaced by a real-AI twin (same
    // body tag, real entity type, no puppet marker). Per actor, and the entity
    // type asserted is THIS actor's — which is precisely what one exemplar could
    // never have covered.
    for a in actors {
        let safe = plan::safe_local(a.id.as_str());
        let mut b = packtest_header(&format!(
            "{}: unleash-actor `{}` swaps the puppet for a real-AI twin",
            artifact_title(c),
            a.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!(
            "execute store result score #pup_unl_{safe} dw.sys if entity @e[tag=dw_pup_{safe}]"
        ));
        b.push(format!("assert score #pup_unl_{safe} dw.sys matches 1"));
        b.push(format!("function {ns}:unleash_{safe}"));
        // puppet marker gone, one twin of the real entity type remains.
        b.push(format!(
            "execute store result score #pup2_unl_{safe} dw.sys if entity @e[tag=dw_pup_{safe}]"
        ));
        b.push(format!("assert score #pup2_unl_{safe} dw.sys matches 0"));
        b.push(format!(
            "execute store result score #twin_unl_{safe} dw.sys if entity @e[type={},tag=dw_actor_{safe}]",
            a.entity
        ));
        b.push(format!("assert score #twin_unl_{safe} dw.sys matches 1"));
        // The twin is this test's residue — without this kill it survives the
        // test, and any later spawn no-ops against its body tag while owning no
        // puppet marker (the exact v06_spawn_idempotent red).
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        write(&format!("v06_unleash_{safe}"), b);
    }

    // The puppets whose leg drivers carry the supersession guard: a template that
    // invokes one of their drivers directly claims it first ([`walk_claim`]).
    let actor_guarded = supersedable_walkers(actor_moves.iter().map(|m| m.actor.as_str()));
    // move-actor: fast-forward the driver to its final waypoint (running on_arrive on
    // that same tick) and assert the puppet is at the destination cell.
    if let Some(m) = actor_moves.first() {
        let safe = plan::safe_local(&m.actor);
        let bare = moveactor_bare(&m.actor, &m.to, &m.gate_key);
        let total = m.ticks();
        let p = m.target;
        let mut b = packtest_header(&format!(
            "{}: move-actor walks its puppet to the destination cell",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.extend(walk_claim(
            Walker::Actor,
            &bare,
            &safe,
            actor_guarded.contains(m.actor.as_str()),
        ));
        b.push(format!("scoreboard players set #at_{bare} dw.sys {total}"));
        b.push(format!("function {ns}:ma_tick_{bare}"));
        b.push(format!(
            "execute store result score #arr_mvac dw.sys if entity @e[tag=dw_pup_{safe},x={},dx=0,y={},dy=0,z={},dz=0]",
            p[0], p[1], p[2]
        ));
        b.push("assert score #arr_mvac dw.sys matches 1..".to_string());
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        write("v06_move_actor", b);
    }

    // Walker→NPC handoff (round-6 island QA): the first move-actor whose
    // on_arrive fires a `spawn-npc` is a scene handoff — a scripted puppet
    // walks in, vanishes, and the real (dialogue-bearing) NPC takes its place.
    // The delve soft-locks if the handoff leaves the puppet standing or the NPC
    // short an entity, so pin it end to end: drive the arrival tick and assert
    // puppet gone, NPC body present, and exactly one interaction hitbox. Every
    // campaign gate is sealed first (its `close-gate` fill): the island beat
    // fires this handoff with the boulder down, and arrival must be immune to
    // sealed terrain — the driver is a tp chain, not pathfinding. Gates are
    // re-opened afterwards (fill air replace <block>), so the template leaves
    // no block residue for a sibling (batch model).
    let handoff = actor_moves.iter().find_map(|m| {
        all_campaign_effects(c)
            .into_iter()
            .find_map(|e| match &e.verb {
                Verb::MoveActor {
                    actor,
                    to,
                    on_arrive,
                    ..
                } if actor.as_str() == m.actor && *to == m.to => on_arrive
                    .iter()
                    .find_map(|a| match &a.verb {
                        Verb::SpawnNpc { npc, .. } => Some(npc.as_str().to_string()),
                        _ => None,
                    })
                    .map(|npc| (m, npc)),
                _ => None,
            })
    });
    if let Some((m, npc_id)) = handoff
        && let Some(npc_tag) = plan
            .npcs
            .iter()
            .find(|n| n.npc_id == npc_id)
            .map(|n| n.tag.clone())
    {
        let safe = plan::safe_local(&m.actor);
        let bare = moveactor_bare(&m.actor, &m.to, &m.gate_key);
        let total = m.ticks();
        // Every distinct gate a `close-gate` effect seals, in first-appearance
        // order (deterministic).
        let mut sealed: Vec<(&[i32; 3], &[i32; 3], &String)> = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for e in all_campaign_effects(c) {
            if let Verb::CloseGate { anchor, .. } = &e.verb
                && !seen.contains(&anchor.as_str())
            {
                seen.push(anchor.as_str());
                for ((_, name), resolved) in &plan.anchors {
                    if name == anchor.as_str()
                        && let ResolvedAnchor::Gate { from, to, block } = resolved
                    {
                        sealed.push((from, to, block));
                    }
                }
            }
        }
        let mut b = packtest_header(&format!(
            "{}: move-actor arrival hands off to NPC `{npc_id}` with every gate sealed",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("kill @e[tag={npc_tag}]"));
        for (from, to, block) in &sealed {
            b.push(format!(
                "fill {} {} {} {} {} {} {}",
                from[0], from[1], from[2], to[0], to[1], to[2], block
            ));
        }
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.extend(walk_claim(
            Walker::Actor,
            &bare,
            &safe,
            actor_guarded.contains(m.actor.as_str()),
        ));
        b.push(format!("scoreboard players set #at_{bare} dw.sys {total}"));
        b.push(format!("function {ns}:ma_tick_{bare}"));
        b.push(format!(
            "execute store result score #pup_ahof dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #pup_ahof dw.sys matches 0".to_string());
        b.push(format!(
            "execute store result score #npc_ahof dw.sys if entity @e[tag=dw_npc,tag={npc_tag}]"
        ));
        b.push("assert score #npc_ahof dw.sys matches 1".to_string());
        b.push(format!(
            "execute store result score #box_ahof dw.sys if entity @e[type=minecraft:interaction,tag={npc_tag}]"
        ));
        b.push("assert score #box_ahof dw.sys matches 1".to_string());
        // No residue: NPC out, actor tag out, gates back open.
        b.push(format!("kill @e[tag={npc_tag}]"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        for (from, to, block) in &sealed {
            b.push(format!(
                "fill {} {} {} {} {} {} minecraft:air replace {}",
                from[0], from[1], from[2], to[0], to[1], to[2], block
            ));
        }
        write("v06_arrive_handoff", b);
    }
}
