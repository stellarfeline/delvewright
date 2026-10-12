//! Actors: the puppet and its twin, equipment, despawn, move-actor, and the actor functions.

use super::*;

/// Emit a `despawn-actor` inline (spec-0014). Both styles target the actor body
/// tag `dw_actor_<id>` (so a puppet **or** an unleashed twin is removed —
/// re-caging is despawn + spawn) through [`removal_lines`]: `kill` is the
/// author's on-screen death ([`Exit::OnScreen`]), `vanish` leaves unseen
/// ([`Exit::Unseen`]).
pub(super) fn emit_despawn_actor(
    ns: &str,
    actor: &str,
    style: delvewright_dsl::DespawnStyle,
    declares_drops: bool,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::DespawnStyle;
    let exit = match style {
        DespawnStyle::Kill => Exit::OnScreen,
        DespawnStyle::Vanish => Exit::Unseen,
    };
    let tag = format!("dw_actor_{}", plan::safe_local(actor));
    body.extend(removal_lines(ns, &tag, declares_drops, exit));
}

/// The spawn yaw for an actor from its `facing` (default south = 0).
pub(super) fn actor_facing_yaw(a: &delvewright_dsl::Actor) -> i32 {
    a.facing.map(|f| facing_yaw(Some(f.token()))).unwrap_or(0)
}

/// The `/summon` command for an actor's caged puppet (spec-0014). NoAI/Silent/
/// no-loot (`DeathLootTable` empty), tag `dw_actor` + `dw_actor_<id>` + a
/// puppet-only `dw_pup_<id>` marker (so `unleash`/`move` target the puppet without
/// touching a real-AI twin). `Invulnerable` unless `vulnerable`; a vulnerable puppet
/// stays knockback-immune (`knockback_resistance` 1.0) — the tower-defense creep. A
/// `skin` re-dresses it as a `minecraft:mannequin`, exactly as a stage-2 NPC.
///
/// **An actor is a body, and a `skin` is a costume.** What the two branches
/// differ over is only what the costume forces: the entity id, the field the
/// label rides (a mannequin's `description`, a mob's `CustomName`), and how a
/// scripted body is held still (`immovable` against
/// `NoAI`/`NoGravity`/`PersistenceRequired`). Everything the author declared
/// about the *body* — `vulnerable`, `attributes`, `equipment` — is computed once,
/// above the branch, and spliced into both, so a property cannot be carried by
/// one dress and lost by the other. The loot half is not the compiler's choice:
/// see [`body_carries_loot_nbt`].
pub(super) fn actor_puppet_summon(
    ns: &str,
    a: &delvewright_dsl::Actor,
    pos: [i32; 3],
    yaw: i32,
) -> String {
    let safe = plan::safe_local(a.id.as_str());
    let p = ent_xyz(pos);
    // spec-0101: a watching puppet carries the live-watch tag from its summon.
    let watch_tag = if a.watch.is_some() {
        format!(",\"{}\"", crate::compiler::watching::WATCH_TAG)
    } else {
        String::new()
    };
    let tags = format!("Tags:[\"dw_actor\",\"dw_actor_{safe}\",\"dw_pup_{safe}\"{watch_tag}]");
    // The body that actually ships — the ONE authority both the router and the
    // emitter ask, so "which entity is this puppet" is answered in one place.
    let body = crate::compiler::nav::actor_body_entity(a);
    let inv = if a.vulnerable { 0 } else { 1 };
    // Compiler-owned knockback-immunity first (a `vulnerable` puppet is a
    // damageable creep, never a shovable one), then whatever the author
    // declared — so a puppet with no `attributes` renders exactly the
    // pre-`attributes` string and every earlier campaign stays byte-identical.
    let mut entries: Vec<String> = Vec::new();
    if a.vulnerable {
        entries.push("{id:\"minecraft:knockback_resistance\",base:1.0}".to_string());
    }
    entries.extend(attribute_entries(a.attributes.as_ref()));
    let attrs = wrap_attribute_entries(entries);
    // spec-0021: actor gear rides on BOTH the puppet and the twin, so the
    // dormant elite the party circles is visibly the thing that stands up.
    let equip = actor_equipment(a, &body)
        .map(|e| format!(",{e}"))
        .unwrap_or_default();
    if let Some(skin) = &a.skin {
        let desc = a
            .name
            .as_deref()
            .unwrap_or_else(|| a.id.as_str().rsplit('/').next().unwrap_or("actor"));
        format!(
            "summon {body} {} {} {} {{profile:{{texture:\"delvewright:npc/{}\",model:\"{}\"}}{},immovable:1b,pose:\"standing\",Invulnerable:{inv}b,Silent:1b,Rotation:[{yaw}f,0f],description:{},{tags}{attrs}{equip}}}",
            p[0],
            p[1],
            p[2],
            skin.texture_id,
            skin.model.token(),
            mannequin_hidden_layers_nbt(skin),
            snbt_text_component(desc)
        )
    } else {
        let name = a
            .name
            .as_deref()
            .map(|n| format!(",CustomName:{},CustomNameVisible:1b", snbt_component(n)))
            .unwrap_or_default();
        let pose = mannequin_pose_nbt(&body);
        // v0.9: a declared quest-item drop points the field the puppet
        // has always carried at a table the compiler emits. `unleash` and
        // `despawn-actor` strip it again ([`strip_drops_line`]) — only a player's
        // kill yields it.
        let loot = death_loot_table(
            ns,
            has_item_drop(&a.drops).then(|| drop_loot_path("actor", a.id.as_str())),
        );
        format!(
            "summon {body} {} {} {} {{NoAI:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b{pose},Invulnerable:{inv}b,DeathLootTable:\"{loot}\",Rotation:[{yaw}f,0f],{tags}{name}{attrs}{equip}}}",
            p[0], p[1], p[2]
        )
    }
}

/// Whether a body of this entity kind carries the `Mob`-only loot NBT the
/// compiler writes for a declared `drops` — `DeathLootTable` and `drop_chances`.
///
/// `minecraft:mannequin` is a `LivingEntity` and not a `Mob`, so neither field is
/// part of its save data: vanilla accepts them in the `/summon` compound, reads
/// them with nothing, and persists nothing. Live A/B on the pinned 1.21.11
/// server — a mannequin summoned with `DeathLootTable:"minecraft:empty"`,
/// `drop_chances:{…}`, `equipment:{…}` and `attributes:[…]` reads back
/// `equipment` verbatim and `attributes` merged over its defaults
/// (`max_health` 40 ⇒ `Health: 40.0f`), and answers `Found no elements matching`
/// for `DeathLootTable` and for `drop_chances`. Damaged to death wearing that
/// gear it drops **nothing**, so the no-grind invariant the `drop_chances` zeros
/// exist to hold is held by the body itself rather than by a field it ignores.
///
/// So a skinned actor's `drops` reaches the player through the unleashed twin —
/// a real `Mob` — and not through the caged mannequin. That is a vanilla limit,
/// not an emission choice, and emitting the two fields anyway would be a
/// statement the world does not carry.
pub(super) fn body_carries_loot_nbt(entity: &str) -> bool {
    entity.strip_prefix("minecraft:").unwrap_or(entity) != "mannequin"
}

/// The `pose` NBT field a `minecraft:mannequin` needs, or `""` for any other
/// entity — spliced into every summon whose entity id is author-supplied.
///
/// A mannequin summoned without an explicit `pose` serializes it as `DYING`, which
/// the server then fails to encode at save time (`Failed to encode value 'DYING'`
/// in a PackTest world's teardown log) and which is simply wrong data for a
/// standing figure. The skinned NPC/actor paths hardcode `minecraft:mannequin` and
/// have always emitted it; the paths that take the entity id **from content**
/// (`npc.base_entity`, `actor.entity`, and the `unleash` twin, which has no skin
/// branch at all) did not — so an author who spelled `minecraft:mannequin` by hand
/// got the broken pose. Valid 1.21.11 mannequin poses: standing, crouching,
/// swimming, fall_flying, sleeping (spec-0009 template).
pub(super) fn mannequin_pose_nbt(entity: &str) -> &'static str {
    let id = entity.strip_prefix("minecraft:").unwrap_or(entity);
    if id == "mannequin" {
        ",pose:\"standing\""
    } else {
        ""
    }
}

/// The state vanilla's `finalizeSpawn` would have given this entity, spliced into
/// every summon the compiler writes with an NBT compound — or `""` for a species
/// that needs none.
///
/// **The trap this closes** (round-8 island QA, proven on a live pinned 1.21.11
/// server). `/summon <entity> <pos>` calls the mob's `finalizeSpawn`;
/// `/summon <entity> <pos> <nbt>` — *any* NBT compound, even `{}` — does **not**.
/// The compiler always passes NBT (tags are how every entity it owns is addressed),
/// so every mob it summons is spawned un-finalized. For most species that is
/// invisible. For `minecraft:warden` it is fatal: `finalizeSpawn` is the only place
/// the `minecraft:dig_cooldown` brain memory is seeded, and a warden whose brain
/// lacks it enters the DIG activity on its first AI tick, plays the burrow
/// animation, and despawns about five seconds later. That is exactly what the
/// owner saw — strike the sleeping giant, watch him turn into a warden, watch the
/// warden immediately dig itself back into the ground.
///
/// Live A/B on the pinned server:
/// `summon minecraft:warden <pos>` → `Brain{memories:{"minecraft:dig_cooldown":{value:{},ttl:1200L}}}`;
/// `summon minecraft:warden <pos> {}` → `Brain{memories:{}}`, gone in ~5s.
///
/// The fix is to write the same data vanilla would have written — the entity's own
/// documented, codec-backed NBT, not a workaround for a missing primitive. The
/// warden refreshes the cooldown itself every tick it is awake and doing anything,
/// so seeding vanilla's own 1200-tick value is enough to keep an unleashed boss in
/// the world for as long as the campaign wants it (verified: still present and
/// roaming past 80 s, `ttl` held at 1199 by the warden's own AI).
///
/// Only species whose un-finalized state is actually *wrong* appear here, so every
/// campaign without one stays byte-identical.
pub(super) fn spawn_finalize_nbt(entity: &str) -> &'static str {
    match entity.strip_prefix("minecraft:").unwrap_or(entity) {
        // `Warden.finalizeSpawn` → `setMemoryWithExpiry(DIG_COOLDOWN, Unit, 1200)`.
        "warden" => ",Brain:{memories:{\"minecraft:dig_cooldown\":{value:{},ttl:1200L}}}",
        _ => "",
    }
}

/// The `/summon` command for an actor's real-AI twin (spec-0014 `unleash`): the
/// real `entity` with AI enabled, same name and body tag (`dw_actor` +
/// `dw_actor_<id>`), but **no** `dw_pup_<id>` marker — so killing the puppet by
/// its marker leaves the twin fighting.
///
/// `at` is the position argument: `~ ~ ~` for the unleash (run `execute at` the
/// puppet, so the twin stands up exactly where the puppet knelt), or the actor's
/// absolute origin cell for the bonfire's undefeated re-seat, which has no puppet
/// left to stand at. One string, so the two paths can never drift into two
/// different bodies.
///
/// The twin is the compiler's only *free-AI* summon, so it is where
/// [`spawn_finalize_nbt`] matters: a caged puppet is `NoAI`, and a `NoAI` mob never
/// runs `customServerAiStep`, which is why the island's herdsman warden could stand
/// in the meadow indefinitely while the unleashed one burrowed away.
pub(super) fn actor_twin_summon(ns: &str, a: &delvewright_dsl::Actor, at: &str) -> String {
    let safe = plan::safe_local(a.id.as_str());
    let name = a
        .name
        .as_deref()
        .map(|n| format!(",CustomName:{},CustomNameVisible:1b", snbt_component(n)))
        .unwrap_or_default();
    let pose = mannequin_pose_nbt(&a.entity);
    let finalize = spawn_finalize_nbt(&a.entity);
    // The twin inherits the puppet's gear: unleashing swaps the body, not the
    // costume. Drop chances stay 0 — killing the elite must never drop its kit.
    let equip = actor_equipment(a, &a.entity)
        .map(|e| format!(",{e}"))
        .unwrap_or_default();
    // The twin inherits the puppet's tuning too: the whole point of an elite's
    // `attributes` is the body that actually fights, and unleashing replaces the
    // body. Knockback-immunity deliberately does NOT ride along — that is the
    // caged creep's property, not the freed elite's.
    let attrs = attributes_snbt(a.attributes.as_ref());
    // The twin's body is `entity` as written, so an author who spelled
    // `minecraft:mannequin` there gets a twin with no reader for a death loot
    // table — the same vanilla limit the puppet branch states.
    let loot = if body_carries_loot_nbt(&a.entity) {
        let path = death_loot_table(
            ns,
            has_item_drop(&a.drops).then(|| drop_loot_path("actor", a.id.as_str())),
        );
        format!(",DeathLootTable:\"{path}\"")
    } else {
        String::new()
    };
    format!(
        "summon {} {at} {{PersistenceRequired:1b{pose}{loot},Tags:[\"dw_actor\",\"dw_actor_{safe}\"]{name}{finalize}{attrs}{equip}}}",
        a.entity
    )
}

/// The `equipment`/`drop_chances` SNBT fragment for an actor (no leading comma),
/// or `None` when the actor declares no gear. `body` is the entity id the gear is
/// being hung on — the puppet's ([`crate::compiler::nav::actor_body_entity`], a mannequin when the
/// actor declares a `skin`) or the twin's (`actor.entity`) — because
/// `drop_chances` is `Mob` save data and a mannequin is not a `Mob`
/// ([`body_carries_loot_nbt`]).
///
/// Deliberately NOT the wave path's [`wave_equipment`]: that function falls back
/// to the armed-mob default table, which would silently arm every actor whose
/// entity happens to be a vindicator or skeleton and break byte-identity for
/// every campaign authored before this field existed. An actor is a directed
/// set piece — it wears exactly what the author declared, and nothing when they
/// declared nothing.
pub(super) fn actor_equipment(a: &delvewright_dsl::Actor, body: &str) -> Option<String> {
    let eq = a.equipment.as_ref()?;
    let declared = declared_drop_slots(&a.drops);
    let mut items: Vec<String> = Vec::new();
    let mut chances: Vec<String> = Vec::new();
    // Fixed emission order, [`EquipSlot::ALL`]'s, matching the wave path
    // (ADR-0006 determinism).
    for (slot, piece) in eq.pieces() {
        if let Some(p) = piece {
            let key = slot.nbt();
            let comps = enchantment_components(p);
            items.push(format!("{key}:{{id:\"{}\",count:1{comps}}}", p.item()));
            chances.push(format!("{key}:{}", drop_chance_for(key, &declared)));
        }
    }
    if items.is_empty() {
        return None;
    }
    if !body_carries_loot_nbt(body) {
        // A mannequin wears the gear and drops none of it, whatever is written
        // into a field it has no reader for.
        return Some(format!("equipment:{{{}}}", items.join(",")));
    }
    Some(format!(
        "equipment:{{{}}},drop_chances:{{{}}}",
        items.join(","),
        chances.join(",")
    ))
}

/// Warden anger at which `AngerLevel` is `ANGRY` and the mob commits to a target.
/// Vanilla's own maximum (`AngerManagement`), so the lock is immediate and total.
pub(super) const WARDEN_MAX_ANGER: i32 = 150;

/// The lines that lock an unleashed twin's aggression onto the player who struck
/// the trigger: a hostile that a player *provoked* must
/// come for that player, not wander off looking for someone.
///
/// **Only species with a proven vanilla primitive get one.** `minecraft:warden`
/// persists its target list as `anger.suspects` (`AngerManagement`), a codec-backed
/// field vanilla itself round-trips, and seeding it works end to end on a live
/// pinned 1.21.11 server: the warden left its spawn cell, closed on the seeded
/// player's position and killed that player.
///
/// The `NeutralMob` pair (`AngerTime` / `AngryAt`) looks like the same primitive for
/// endermen, piglins, wolves and friends, and was tried — but on 1.21.11 neither
/// field reads back after a tick, for any of the species tested, with a real online
/// player's UUID or a synthetic one. Whatever the mechanism (the codec dropping
/// defaults, or `updatePersistentAnger` clearing the target it just resolved), the
/// data does not survive, so the compiler does not pretend it does: every non-warden
/// species is left to vanilla's own nearest-player acquisition, and that limit is
/// documented rather than papered over (CLAUDE.md: no hacks at any layer — if the
/// primitive is not really there, the feature does not get faked downstream).
///
/// Guarded on the striker storage actually holding a UUID, so an `unleash-actor`
/// fired from anywhere other than a click trigger's own bundle changes nothing.
pub(super) fn aggro_lock_lines(entity: &str, safe: &str) -> Vec<String> {
    let id = entity.strip_prefix("minecraft:").unwrap_or(entity);
    if id != "warden" {
        return Vec::new();
    }
    // The twin is the only entity left wearing the body tag: `unleash_<id>` kills
    // the puppet on the line before these run.
    let target = format!("@e[tag=dw_actor_{safe},limit=1]");
    let guard = format!("execute if data storage {STRIKER_STORAGE} {STRIKER_PATH} run");
    // `AngerManagement`: a suspect list of `{uuid, anger}`. Seed one suspect at
    // vanilla's maximum anger, then overwrite its placeholder UUID from storage —
    // `data modify … set from storage` cannot create the list element, so the
    // element is written first and patched second.
    vec![
        format!(
            "{guard} data modify entity {target} anger.suspects set value [{{anger:{WARDEN_MAX_ANGER},uuid:[I;0,0,0,0]}}]"
        ),
        format!(
            "{guard} data modify entity {target} anger.suspects[0].uuid set from storage {STRIKER_STORAGE} {STRIKER_PATH}"
        ),
    ]
}

/// The generated start-function name for a `move-actor` (content key).
pub(super) fn moveactor_fn(actor: &str, to: &delvewright_dsl::Mark, gate_key: &str) -> String {
    format!("ma_{}_{}{gate_key}", plan::safe_local(actor), mark_key(to))
}

/// The scoreboard-safe suffix shared by a move-actor's driver functions/sentinels.
pub(super) fn moveactor_bare(actor: &str, to: &delvewright_dsl::Mark, gate_key: &str) -> String {
    moveactor_fn(actor, to, gate_key)
        .strip_prefix("ma_")
        .unwrap_or("move")
        .to_string()
}

/// Actor staging functions (spec-0014): a `spawn_actor_<id>` (idempotent summon) and
/// `unleash_<id>` (puppet → real-AI twin) per declared actor, plus a per-tick
/// teleport driver (with tangent yaw and an `on_arrive` bundle) per planned
/// `move-actor`. Empty for a campaign with no actors (pre-0.6 byte-identical).
///
/// # Supersession — one puppet, one live leg driver
///
/// A `move-actor` driver carries the identical defect `move-npc` had: its
/// re-entry latch `#arun_<bare>` is keyed per **(actor, to, gate)**, so it only
/// ever stopped a leg from restarting *itself*. Two overlapping legs on ONE puppet left
/// two live drivers both `tp`-ing the same body every tick; they fought, and the longer
/// leg — outliving the shorter — wrote the final position, parking the puppet at the
/// FIRST leg's endpoint permanently.
///
/// The contract is the same one `movenpc_fns` documents at length: **last fired wins**,
/// carried by a per-puppet leg generation `#agen_<actor>` stamped onto each driver's
/// `#aown_<bare>`. A driver whose stamp is behind the generation drops its latch and
/// returns without teleporting, arriving, or rescheduling — so it dies on its next
/// scheduled tick. The staleness test is the positive `if own < gen` for the same
/// reason: with both scores unset a score comparison is *false*, so the
/// unfired-generation case reads as "not stale" and the leg runs. A PackTest that
/// invokes a guarded driver directly (`v06_move_actor`, `v06_arrive_handoff`) claims it
/// first ([`walk_claim`]).
///
/// A puppet with only one planned leg can never be superseded and carries none of this,
/// so pre-existing single-leg campaigns stay byte-identical (ADR-0006) — pinned
/// verbatim by `move_supersede.rs`'s `GOLDEN_ONE_LEG`.
pub(super) fn actor_fns(
    plan: &Plan,
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for a in &plan.campaign.quests.content.actors {
        let safe = plan::safe_local(a.id.as_str());
        let Some(pos) = plan.body_point(delvewright_dsl::BodyRef::Actor(a)) else {
            continue; // resolution guaranteed by check_actor_placement (DW0325)
        };
        let yaw = actor_facing_yaw(a);
        // spec-0073: every function that summons this actor's bodies re-captures
        // its bar's max. Absent without a bar → byte-identical.
        let capture = crate::compiler::healthbar::actor_bar(plan.campaign, a.id.as_str())
            .map(|b| b.capture_call(ns));
        let mut spawn = vec![format!(
            "execute unless entity @e[tag=dw_actor_{safe}] run {}",
            actor_puppet_summon(ns, a, pos, yaw)
        )];
        spawn.extend(capture.clone());
        out.push((format!("spawn_actor_{safe}"), lines(&spawn)));
        let mut unleash = vec![format!(
            "execute at @e[tag=dw_pup_{safe},limit=1] run {}",
            actor_twin_summon(ns, a, "~ ~ ~")
        )];
        // The cage leaves unseen: the elite standing up is the twin, and a
        // puppet dying beside it is a death the story never wrote. A puppet
        // carrying a declared drop is stripped first — the twin is the body
        // that owes the player a prize.
        unleash.extend(removal_lines(
            ns,
            &format!("dw_pup_{safe}"),
            actor_declares_drops(a),
            Exit::Unseen,
        ));
        if campaign_captures_striker(plan.campaign) {
            unleash.extend(aggro_lock_lines(&a.entity, &safe));
        }
        unleash.extend(capture.clone());
        out.push((format!("unleash_{safe}"), lines(&unleash)));
        // spec-0074: an actor's `on_kill` — the kill advancement's reward (pay,
        // then re-arm) and the bundle. Absent → byte-identical.
        if let Some(ok) = &a.on_kill {
            let fight = delvewright_dsl::Fight::Actor(a);
            out.push((
                format!("ka_reward_{safe}"),
                lines(&[
                    on_kill_call(ns, fight, ok),
                    format!("advancement revoke @s only {ns}:ka_{safe}"),
                ]),
            ));
            out.push((on_kill_function(fight), on_kill_body(plan, fight, ok)));
        }
        // spec-0016 §1: the UNDEFEATED re-seat. A rest
        // (and a death-respawn at the same fire) deletes the elite the party is
        // still fighting and stands a FRESH body on its origin anchor: full
        // health, no accumulated chip damage, and — the reported regression — back
        // where it belongs instead of wherever the chase left it.
        //
        // Deliberately not `unleash_<id>`: there is no puppet to stand up from,
        // and re-caging one would be worse than doing nothing, because an
        // `unleash-actor` beat fires from a one-shot trigger the engine never
        // re-arms — a re-caged elite would be dormant, `Invulnerable` scenery for
        // the rest of the delve. It comes back as what it was: a freed body on its
        // own ground.
        //
        // The striker aggro lock is deliberately NOT re-applied. Nobody has
        // provoked this body yet, and spec-0016 §1's stationed rule is that
        // nothing a rest puts back may pursue across the map: it stands on its
        // anchor under vanilla-local AI, inside the `follow_range` `DW0478`
        // measured the bonfire against.
        //
        // Emitted only for an actor the campaign unleashes AND a campaign with a
        // bonfire ([`Plan::reseat_actors`]) → byte-identical everywhere else.
        if plan.reseat_actors().iter().any(|r| r.id == a.id) {
            let p = ent_xyz(pos);
            let mut restand = removal_lines(
                ns,
                &format!("dw_actor_{safe}"),
                actor_declares_drops(a),
                Exit::Unseen,
            );
            restand.push(actor_twin_summon(
                ns,
                a,
                &format!("{} {} {}", p[0], p[1], p[2]),
            ));
            restand.extend(capture.clone());
            out.push((format!("actor_restand_{safe}"), lines(&restand)));
        }
    }
    // move-actor per-tick drivers.
    //
    // A puppet with two or more legs can be caught mid-flight by a later one ⇒ its
    // drivers carry the generation guard (see the supersession section of
    // `movenpc_fns`).
    let guarded = supersedable_walkers(actor_moves.iter().map(|m| m.actor.as_str()));
    for m in actor_moves {
        let safe = plan::safe_local(&m.actor);
        let bare = moveactor_bare(&m.actor, &m.to, &m.gate_key);
        let total = m.ticks();
        let supersedable = guarded.contains(m.actor.as_str());
        // `#aown_<bare> < #agen_<actor>` ⇔ a later leg for this puppet has started.
        let (own, gen_) = walk_stamp(Walker::Actor, &bare, &safe);
        let stale = format!("score {own} dw.sys < {gen_} dw.sys");
        // The on_arrive bundle for this (actor, to) — the first-seen effect,
        // matching the planner's dedup order.
        let on_arrive: &[QuestEffect] = all_campaign_effects(plan.campaign)
            .into_iter()
            .find_map(|e| match &e.verb {
                Verb::MoveActor {
                    actor,
                    to,
                    on_arrive,
                    ..
                } if actor.as_str() == m.actor && *to == m.to => Some(on_arrive.as_slice()),
                _ => None,
            })
            .unwrap_or(&[]);

        // start: guard re-entry, take the walk generation, reset the tick counter,
        // schedule the driver. The re-entry refusal is generation-aware: a latch left
        // armed by a driver this puppet has already superseded must not block the
        // re-fire of that same leg (it is itself a later leg, and wins).
        let mut start = Vec::new();
        if supersedable {
            start.push(format!(
                "execute if score #arun_{bare} dw.sys matches 1 unless {stale} run return fail"
            ));
            start.push(format!("scoreboard players add {gen_} dw.sys 1"));
            start.push(format!(
                "scoreboard players operation {own} dw.sys = {gen_} dw.sys"
            ));
        } else {
            start.push(format!(
                "execute if score #arun_{bare} dw.sys matches 1 run return fail"
            ));
        }
        // spec-0101: a watching puppet yields its yaw to the walk for as long
        // as the walk runs; the arrival tick below hands it back.
        let watches = crate::compiler::watching::body_watches(plan, &m.actor);
        let body_sel = format!("tag=dw_pup_{safe}");
        if watches {
            start.push(crate::compiler::watching::yield_line(&body_sel));
        }
        start.push(format!("scoreboard players set #arun_{bare} dw.sys 1"));
        start.push(format!("scoreboard players set #at_{bare} dw.sys 0"));
        start.push(format!("schedule function {ns}:ma_tick_{bare} 1t"));
        out.push((moveactor_fn(&m.actor, &m.to, &m.gate_key), lines(&start)));

        let mut tick: Vec<String> = Vec::new();
        if supersedable {
            // Superseded: drop the latch (so this leg can be fired again later) and
            // stop — no teleport, no arrive hook, no reschedule. The `schedule` this
            // driver queued before it lost the puppet is what brought us here; not
            // rescheduling is what ends it.
            tick.push(format!(
                "execute if {stale} run scoreboard players set #arun_{bare} dw.sys 0"
            ));
            tick.push(format!("execute if {stale} run return fail"));
        }
        for (t, (w, y)) in m.waypoints.iter().zip(m.yaws.iter()).enumerate() {
            tick.push(format!(
                "execute if score #at_{bare} dw.sys matches {t} run tp @e[tag=dw_pup_{safe}] {} {} {} {y} 0",
                fmt_f64(w[0]),
                fmt_f64(w[1]),
                fmt_f64(w[2])
            ));
        }
        if watches {
            tick.push(crate::compiler::watching::resume_line(
                &format!("score #at_{bare} dw.sys matches {total}"),
                &body_sel,
            ));
        }
        if !on_arrive.is_empty() {
            tick.push(format!(
                "execute if score #at_{bare} dw.sys matches {total} run function {ns}:ma_arrive_{bare}"
            ));
        }
        tick.push(format!("scoreboard players add #at_{bare} dw.sys 1"));
        tick.push(format!(
            "execute if score #at_{bare} dw.sys matches {}.. run scoreboard players set #arun_{bare} dw.sys 0",
            total + 1
        ));
        tick.push(format!(
            "execute unless score #at_{bare} dw.sys matches {}.. run schedule function {ns}:ma_tick_{bare} 1t",
            total + 1
        ));
        out.push((format!("ma_tick_{bare}"), lines(&tick)));

        if !on_arrive.is_empty() {
            // Server command source (see `Audience`): `ma_tick_<bare>` runs from
            // the scheduler, so `@s` is unbound in everything it calls.
            let arrive = emit_effect_bundle(plan, on_arrive, Audience::Scheduled);
            out.push((format!("ma_arrive_{bare}"), lines(&arrive)));
        }
    }
    out
}
