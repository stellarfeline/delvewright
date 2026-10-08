//! Checkpoints, bonfires, party wipe and the bonfire mend (spec-0012, spec-0016 §1).

use super::*;

/// Emit a `set-checkpoint` (DSL v0.6, spec-0012): the party-wide vanilla
/// `spawnpoint @a`, the `storage dw:cp pos` mirror other features read
/// (spec-0013 boundary return), and — when any checkpoint carries an
/// `on_respawn` hook — the active-checkpoint marker `#cp dw.sys` the respawn
/// dispatcher keys on. Party-wide via the explicit `@a`, regardless of the
/// caller's `@s` context.
pub(super) fn emit_set_checkpoint(
    plan: &Plan,
    anchor: &str,
    on_respawn: &[QuestEffect],
    body: &mut Vec<String>,
) {
    if let Some(pos) = anchor_point_any(plan, anchor) {
        body.push(format!("spawnpoint @a {} {} {}", pos[0], pos[1], pos[2]));
        body.push(format!(
            "data modify storage dw:cp pos set value [{}, {}, {}]",
            pos[0], pos[1], pos[2]
        ));
        if plan.any_checkpoint() {
            let idx = plan
                .checkpoint_for(anchor, on_respawn)
                .map(|c| c.index)
                .unwrap_or(0);
            body.push(format!("scoreboard players set #cp dw.sys {idx}"));
        }
    }
}

/// **The death-position seam — measured, and the answer is that it needs nothing.**
///
/// Every command this returns is prepended to the corpse-side branch of
/// `cp_respawn_check`, ahead of `on_death_fire`, so whatever records "where the
/// player died" runs before any authored effect can read it. It emits **nothing**,
/// and that is a settled answer rather than a deferral.
///
/// The seam was carved because spec-0032's recovery stake needs the death position
/// and there were two candidate vanilla mechanisms — a pre-respawn death
/// advancement, or the read-only `LastDeathLocation` player NBT — whose behaviour
/// for non-entity deaths (void, fall, drowning) nobody had measured. CLAUDE.md's
/// debug doctrine answers a question like that by measurement, never by recall.
///
/// **Measured, 5 causes × 3 repeats, every repeat agreeing**
/// (`docs/notes/death-and-teleport-spike.md`): the `deathCount` edge arms **on the
/// corpse**, pre-respawn, for void, fall, drowning, lava and a mob kill alike; and
/// the corpse's own position IS the death position, stable for the whole death
/// screen (measured drift 0.000 in all 15 trials — a corpse stops falling).
///
/// So there is nothing to capture. `on_death_fire` already runs `as @s` on that
/// corpse, and `execute at @s` inside it is positioned at the death point by
/// construction — no scratch storage, no NBT read, no extra command in any
/// campaign's tick. An advancement would have been the wrong instrument
/// (`entity_killed_player` fires for one cause of five; `entity_hurt_player` fires
/// on the FIRST damage event with the player still at 16–20 HP, so it means "was
/// hurt", never "died here"), and `LastDeathLocation` would have been a redundant
/// second reading of a position the executor already stands on.
///
/// It stays a named function rather than becoming a comment for the reason it was
/// one to begin with: the alternative — writing nothing and remembering — is how a
/// seam becomes folklore.
pub(super) fn death_position_capture() -> Vec<String> {
    Vec::new()
}

/// One line per checkpoint: seat `@s` on the active checkpoint's cell.
pub(super) fn cp_seat_dispatch(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    plan.checkpoints
        .iter()
        .map(|c| {
            format!(
                "execute if score #cp dw.sys matches {} run function {ns}:cp_seat_{}",
                c.index, c.index
            )
        })
        .collect()
}

/// **The party-wipe detector** (spec-0016 §1, multiplayer): the tick lines that
/// latch `#wipe` and tag every body when no player in the party is alive.
///
/// "Alive" is the very predicate the death edge already uses
/// (`unless data entity @s {Health:0.0f}`), so the pack has one meaning of dead.
/// `@a` matches a corpse on its death screen and never a disconnected player, so
/// the wipe is "every player present is dead", the state in which a party
/// can no longer recover by itself. A wipe stays latched until the first respawn
/// spends it (`cp_respawn_fire`), and `dw_wiped` stays on each body until that
/// body respawns, so a player who dies after a teammate has already come back
/// is not part of the wipe. Empty for a campaign with no bonfire.
///
/// With a declared respawn wait (spec-0077 §4) "alive" is "in play": a player
/// who is waiting ([`RW_CLOCK`] holds a score) counts as down, so a party of
/// two where one waits and the other dies is wiped. The tick also counts the
/// players present, which is what decides whether a fallen player is alone.
/// The wait is read off its clock, not off [`CUTSCENE_TAG`], because a cutscene
/// tags every player and would otherwise latch a wipe.
pub(super) fn party_wipe_tick(plan: &Plan) -> Vec<String> {
    if !wipes(plan) {
        return Vec::new();
    }
    let in_play = if respawn_wait(plan).is_some() {
        format!("unless score @s {RW_CLOCK} matches 1.. ")
    } else {
        String::new()
    };
    let mut out = vec![
        format!("scoreboard players set {ALIVE} dw.sys 0"),
        format!(
            "execute as @a unless data entity @s {{Health:0.0f}} {in_play}run scoreboard players \
             add {ALIVE} dw.sys 1"
        ),
    ];
    if respawn_wait(plan).is_some() {
        out.push(format!(
            "execute store result score {RW_PRESENT} dw.sys if entity @a"
        ));
    }
    out.extend([
        format!(
            "execute if score {ALIVE} dw.sys matches 0 if entity @a run scoreboard players set \
             {WIPE} dw.sys 1"
        ),
        format!("execute if score {ALIVE} dw.sys matches 0 run tag @a add {WIPED}"),
    ]);
    out
}

/// Generate the death-edge functions: the campaign's `on_death` beat (DSL v0.10,
/// spec-0031) and the checkpoint respawn dispatch (DSL v0.6, spec-0012).
///
/// **One detector, two edges.** Death is detected exactly once, by the vanilla
/// `deathCount` criterion (`dw.deaths`), and `cp_respawn_check` is the one
/// function that reads it. What spec-0031 adds is a second *acknowledgement* of
/// that same counter, not a second detector — because the two consumers want
/// opposite sides of the same event:
///
/// * `on_death` wants **the moment of death**: the player is still a corpse on
///   the death screen, standing where they died. `deathCount` has already ticked
///   up there (measured — it is why the v0.6 half needs its `alive`
///   guard at all), and `@a` matches a corpse, so the corpse side of the edge is
///   reachable with no new machinery.
/// * `on_respawn` and the re-seat want **the player who has come back**, so they
///   hold both their fire and their acknowledgement behind `alive`.
///
/// A single ack cannot serve both: `dw.death_ack` is deliberately withheld while
/// the player is dead, so on the corpse side `deaths > death_ack` stays true for
/// every tick of the death screen. `dw.death_seen` is the corpse-side ack, and it
/// exists only for a campaign that declares `on_death` — a campaign that does not
/// emits this function exactly as it did before the root existed.
///
/// **Not yet emitted: the death POSITION.** See
/// [`death_position_capture`] — the one seam a live measurement fills in.
pub(super) fn emit_checkpoint_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    let on_death = plan.on_death();
    if !plan.any_checkpoint() && on_death.is_empty() {
        return fns;
    }
    // cp_respawn_check (as @s): fire on the death-count edge, then acknowledge.
    //
    // `deathCount` ticks up the moment the player DIES, while they are still on
    // the death screen — a corpse, not a respawned player. Both the re-seat and
    // the authored `on_respawn` bundle belong to the player who has actually come
    // back, so the whole edge (fire AND acknowledge) is held until the player is
    // alive again: a dead player reads `Health: 0.0f`, and holding the ack keeps
    // the edge armed instead of burning it on the corpse.
    let alive = "unless data entity @s {Health:0.0f}";
    let dead = "if data entity @s {Health:0.0f}";
    let mut check: Vec<String> = Vec::new();
    // **The three scores this edge compares have to EXIST before it compares them.**
    // Found live by the bot tier's death-loop stage, which is the only
    // tier that can witness a player death at all; generalised into `DW0495`, which
    // then named a third objective the instance fix had missed.
    //
    // On the pinned 1.21.11 server a scoreboard entry that was never written is
    // NOT zero: every comparison against it is false, so `execute if score @s A >
    // @s B` does not fire when B has no entry (measured — see `crate::compiler::seeding`,
    // which now refuses this shape anywhere in the emitted tree as `DW0495`).
    // `dw.death_ack` and `dw.death_seen` are `dummy` objectives and `dw.deaths` is
    // `deathCount`, and a player who has never died has an entry in none of the
    // three — so the whole edge was dead on a player's FIRST death: no `on_death`
    // (no forfeit, no recovery stake), no `cp_respawn_fire` (no `on_respawn`, no
    // engine re-seat — the party landed wherever vanilla's own `/spawnpoint` hint
    // put them, a hint that cannot be trusted). Both work from the second death
    // onward whatever this does, which is why the gap is invisible to a manual
    // test: every manual test of "does dying work" dies twice.
    //
    // Seeded here rather than at a join hook because this is the one function that
    // reads them, so the two facts cannot drift apart; `add … 0` is idempotent
    // (and, on `deathCount`, does not disturb the criterion — measured: 0 before
    // the first death, 1 after), so running it every tick is a no-op after the
    // first. Emitted only for the edge the campaign declares, so a campaign with
    // neither `on_death` nor a checkpoint moves no byte.
    check.push("scoreboard players add @s dw.deaths 0".to_string());
    if !on_death.is_empty() {
        check.push("scoreboard players add @s dw.death_seen 0".to_string());
    }
    if plan.any_checkpoint() {
        check.push("scoreboard players add @s dw.death_ack 0".to_string());
    }
    // The corpse side FIRST: `on_death` is the earlier moment, and a reader of the
    // generated function should meet the two edges in the order the player lives
    // them. Ordering is otherwise immaterial — the two branches are mutually
    // exclusive by their own guards.
    if !on_death.is_empty() {
        check.extend(death_position_capture());
        check.push(format!(
            "execute {dead} if score @s dw.deaths > @s dw.death_seen run function \
             {ns}:on_death_fire"
        ));
        check.push(format!(
            "execute {dead} run scoreboard players operation @s dw.death_seen = @s dw.deaths"
        ));
    }
    if plan.any_checkpoint() {
        // spec-0077: with a declared wait the edge asks `rw_begin` whether this
        // player waits; it fires `cp_respawn_fire` itself when they do not.
        let on_edge = if respawn_wait(plan).is_some() {
            "rw_begin"
        } else {
            "cp_respawn_fire"
        };
        check.push(format!(
            "execute {alive} if score @s dw.deaths > @s dw.death_ack run function \
             {ns}:{on_edge}"
        ));
        check.push(format!(
            "execute {alive} run scoreboard players operation @s dw.death_ack = @s dw.deaths"
        ));
    }
    fns.push(("cp_respawn_check".to_string(), lines(&check)));
    // on_death_fire (as @s): the campaign's death beat, for the player who died.
    //
    // `Audience::Solo`, the audience `on_respawn` and `on_caught` already use: a
    // death is one player's, and re-broadcasting it to the party would duplicate
    // their narration and their kit.
    if !on_death.is_empty() {
        fns.push((
            "on_death_fire".to_string(),
            lines(&emit_effect_bundle(
                plan,
                on_death,
                root_audience(delvewright_dsl::EffectRootKind::OnDeath),
            )),
        ));
    }
    if !plan.any_checkpoint() {
        return fns;
    }
    // cp_seat_<i> (as @s): put the respawned player ON the checkpoint cell.
    //
    // Why this exists. `set-checkpoint` records the
    // party's respawn with vanilla's `spawnpoint @a <cell>`, but `/spawnpoint` is
    // a *hint*: on death vanilla re-validates the recorded cell and, when the cell
    // or the cell above it is solid or liquid, silently discards it and respawns
    // the player at the WORLD spawn — the campaign entrance. Measured live on
    // 1.21.11: a spawnpoint on a dry cell respawns at `cell + (0.5, 0.1, 0.5)`, the
    // same spawnpoint on a water cell respawns at `setworldspawn`. Past a one-way
    // transport that is not a lost checkpoint, it is an unrecoverable softlock.
    //
    // So the delve stops delegating its own promise. `#cp dw.sys` already names
    // the checkpoint the party last armed; the re-seat teleports the respawned
    // player onto that cell's centre unconditionally. When vanilla honoured the
    // spawnpoint the player is already standing there and the teleport is a no-op
    // they cannot see; when vanilla dropped it, this is the only thing that puts
    // them back. Coordinates are compiled in — no macro, no storage read, so the
    // re-seat cannot itself fail on a malformed mirror.
    for c in &plan.checkpoints {
        fns.push((
            format!("cp_seat_{}", c.index),
            lines(&[format!(
                "tp @s {} {} {}",
                center(c.pos[0]),
                c.pos[1],
                center(c.pos[2])
            )]),
        ));
    }
    // cp_respawn_fire (as @s): dispatch on the active checkpoint.
    let reseat = bonfire_reseat_lines(plan);
    // A bonfire owes the respawning party the same scene reset a rest gives them
    // (spec-0016 §1), so it dispatches even with an empty `on_rest` when there
    // are waves to re-seat. A plain `set-checkpoint` keeps the v0.6 rule exactly.
    let dispatches = |c: &crate::compiler::plan::CheckpointPlan| {
        !c.on_respawn.is_empty() || (c.rest && !reseat.is_empty())
    };
    // The re-seat runs FIRST and for every checkpoint: an `on_respawn` beat that
    // narrates "you wake at the mark" must be read by a player who is on it.
    let mut fire: Vec<String> = cp_seat_dispatch(plan);
    for c in &plan.checkpoints {
        if !dispatches(c) {
            continue;
        }
        fire.push(format!(
            "execute if score #cp dw.sys matches {} run function {ns}:cp_on_respawn_{}",
            c.index, c.index
        ));
    }
    // The wipe is spent by the first respawn after it, whatever checkpoint
    // reigns, and each respawning player's own claim on it is spent with them.
    let wipes = wipes(plan);
    if wipes {
        fire.push(format!("scoreboard players set {WIPE} dw.sys 0"));
        fire.push(format!("tag @s remove {WIPED}"));
    }
    fns.push(("cp_respawn_fire".to_string(), lines(&fire)));
    // party_reseat: the bonfire scene reset's party half — every re-seat, once.
    if wipes && !reseat.is_empty() {
        fns.push(("party_reseat".to_string(), lines(&reseat)));
    }
    // cp_on_respawn_<idx> (as @s): the per-player scene-reset effects.
    for c in &plan.checkpoints {
        if !dispatches(c) {
            continue;
        }
        // `Audience::Solo` (spec-0018): the checkpoint itself is party state, but
        // its `on_respawn` belongs to the ONE player who just died — re-broadcasting
        // it would re-narrate and re-gift every survivor on each death.
        //
        // A bonfire's re-seat (spec-0016 §1) is party state, runs before the
        // bundle so the respawning player's own `on_rest` beats read the restored
        // scene, and runs at most once per party wipe.
        let mut body: Vec<String> = Vec::new();
        if c.rest {
            // **A respawn resets the scene only after a party wipe** (spec-0016
            // §1, multiplayer). One player's death in a party that is still
            // fighting re-seats nothing and runs no `on_rest`: the survivors'
            // fight is left exactly as it stands. When every player was dead at
            // once ([`party_wipe_tick`]), each of them respawns tagged
            // `dw_wiped`; the first one through re-seats the map (`#wipe`
            // is spent in `cp_respawn_fire`), and each runs the fire's `on_rest`
            // as their own. Alone, every death is a wipe, so solo play is the
            // reset-on-every-death loop it always was.
            let mut reset: Vec<String> = Vec::new();
            if !reseat.is_empty() {
                reset.push(format!(
                    "execute if score {WIPE} dw.sys matches 1 run function {ns}:party_reseat"
                ));
            }
            reset.extend(emit_effect_bundle(
                plan,
                &c.on_respawn,
                root_audience(delvewright_dsl::EffectRootKind::DialogueRespawn),
            ));
            if !reset.is_empty() {
                body.push(format!(
                    "execute if entity @s[tag={WIPED}] run function {ns}:cp_reset_{}",
                    c.index
                ));
                fns.push((format!("cp_reset_{}", c.index), lines(&reset)));
            }
            // spec-0016 §1, read forward: death respawns
            // the party at the last-rested bonfire with the same hooks, and vanilla
            // already returns the dead player at full health and hunger. What it
            // does NOT restore is the flask — so without this a player who dies
            // arrives empty-handed at the very bonfire they respawned on and must
            // rest again before they can play. Retry has to be cheap, so a respawn
            // at a bonfire refills the flask exactly as a rest does.
            if !plan.flasks().is_empty() {
                body.push(format!("function {ns}:bonfire_flask"));
            }
        } else {
            body.extend(emit_effect_bundle(
                plan,
                &c.on_respawn,
                root_audience(delvewright_dsl::EffectRootKind::DialogueRespawn),
            ));
        }
        fns.push((format!("cp_on_respawn_{}", c.index), lines(&body)));
    }
    fns
}

/// The re-seat lines a bonfire runs on every rest and on every respawn at it
/// (spec-0016 §1), in a fixed order: the `respawns_on_rest` waves, then the
/// **undefeated** refresh — billed elite/boss waves, then hostile actors. Empty
/// for a campaign that declares none of that surface → byte-identical.
///
/// Two different questions are being asked here, and they take two different
/// primitives.
///
/// * *Has the party MET this wave?* — a scoreboard sentinel
///   ([`wave_seated_holder`]), written by the wave's own `spawn_<wave>`. A
///   `respawns_on_rest` wave comes back whether the party beat it or fled it, so
///   "met" is the only gate, and a wave the delve has not staged yet must not be
///   conjured by a rest.
/// * *Is this thing still STANDING?* — the presence of its own body
///   (`execute if entity`). That is the undefeated test, and it needs no state at
///   all: a boss the party killed leaves no body, so it stays dead by
///   construction (spec-0016 §1), and one they merely chipped is still there, so
///   it is wiped and re-seated whole. `despawn-actor` leaves none either, so a
///   scripted vanish is equally final.
///
/// An actor's line asks the body question twice, because an actor has two
/// postures and only one of them can have been damaged or dragged. A caged
/// puppet (`dw_pup_<id>`) is `NoAI` and knockback-immune — combat cannot move it,
/// and re-seating it would only undo authored `move-actor` staging — so it is
/// left exactly where the campaign put it. An **unleashed twin** is the elite the
/// party is actually fighting: it wears `dw_actor_<id>` and no puppet marker, and
/// the rest deletes it and stands a fresh one on its origin anchor.
pub(super) fn bonfire_reseat_lines(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out: Vec<String> = plan
        .reseat_waves()
        .iter()
        .map(|w| {
            format!(
                "execute if score {} dw.sys matches 1 run function {ns}:wave_reseat_{}",
                wave_seated_holder(w.id.as_str()),
                plan::safe_local(w.id.as_str())
            )
        })
        .collect();
    for w in plan.undefeated_reseat_waves() {
        out.push(format!(
            "execute if entity @e[tag={}] run function {ns}:wave_reseat_{}",
            plan::wave_tag(w.id.as_str()),
            plan::safe_local(w.id.as_str())
        ));
    }
    for a in plan.reseat_actors() {
        let safe = plan::safe_local(a.id.as_str());
        out.push(format!(
            "execute unless entity @e[tag=dw_pup_{safe}] if entity @e[tag=dw_actor_{safe}] \
             run function {ns}:actor_restand_{safe}"
        ));
    }
    out
}

/// Per-tick bonfire **choice** dispatch (spec-0016 §1).
///
/// Right-clicking a bonfire no longer rests: it opens a two-option dialog
/// (`bonfire_open_<i>`, run as the clicking player by the vanilla
/// `player_interacted_with_entity` advancement — the same primitive every
/// interact objective uses). The buttons write the player's answer into the
/// `dw.rest` **trigger** objective, which is the only command surface a
/// non-operator player has, and this tick turns that answer into the chosen
/// function. `dw.rest_at` carries WHICH bonfire the player opened, so a campaign
/// with several bonfires routes each answer to its own rest point.
///
/// `1` = *save only* (move the checkpoint, nothing else), `2` = *rest and save*
/// (the full loop). Empty for a campaign with no bonfire → byte-identical.
pub(super) fn bonfire_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for bf in plan.bonfires() {
        let i = bf.index;
        out.push(format!(
            "execute as @a[scores={{dw.rest=1,dw.rest_at={i}}}] run function {ns}:bonfire_pick_save_{i}"
        ));
        out.push(format!(
            "execute as @a[scores={{dw.rest=2,dw.rest_at={i}}}] run function {ns}:bonfire_pick_rest_{i}"
        ));
    }
    out
}

/// The item modifier a rest mends with: `datapack/data/<ns>/item_modifier/<this>.json`.
pub(super) const BONFIRE_MEND: &str = "bonfire_mend";

/// Every slot a player carries an item in, as `item modify` names it: the four
/// armour slots, the off-hand, and the 36 hotbar (`container.0`–`8`) and
/// inventory (`container.9`–`35`) slots. The main hand is one of the hotbar
/// slots, so it is not named twice.
pub(super) const CARRIED_SLOTS: &[&str] = &[
    "armor.head",
    "armor.chest",
    "armor.legs",
    "armor.feet",
    "weapon.offhand",
    "container.0",
    "container.1",
    "container.2",
    "container.3",
    "container.4",
    "container.5",
    "container.6",
    "container.7",
    "container.8",
    "container.9",
    "container.10",
    "container.11",
    "container.12",
    "container.13",
    "container.14",
    "container.15",
    "container.16",
    "container.17",
    "container.18",
    "container.19",
    "container.20",
    "container.21",
    "container.22",
    "container.23",
    "container.24",
    "container.25",
    "container.26",
    "container.27",
    "container.28",
    "container.29",
    "container.30",
    "container.31",
    "container.32",
    "container.33",
    "container.34",
    "container.35",
];

/// The `set_damage` modifier a rest applies: durability set to full, absolutely
/// (`damage` is the fraction of durability REMAINING; `add` defaults to false).
pub(super) fn bonfire_mend_modifier() -> serde_json::Value {
    json!({"function": "minecraft:set_damage", "damage": 1.0})
}

/// The `bonfire_rest_<i>` functions (spec-0016 §1). Resting is the party-wide
/// event that (a) moves the respawn point to this bonfire — the same three lines
/// a `set-checkpoint` emits, so `dw:cp`, `spawnpoint` and the `#cp` marker stay
/// one shared contract — and (b) runs the `on_rest` scene reset.
///
/// **Audience (spec-0018).** Resting is a **party event** dispatched from the
/// tick, which carries no `@s`, so the bundle is emitted with
/// [`Audience::Scheduled`]: player-facing effects address `@a` — the whole party
/// rests together — and party-state effects name no player and fire once. The
/// respawn path runs the SAME authored effects through `cp_on_respawn_<i>` under
/// [`Audience::Solo`], because a death belongs to the one player who died. That
/// asymmetry is deliberate and is exactly why spec-0016 requires `on_rest` to be
/// idempotent: it is the world's single answer to both a rest and a death, read
/// at two different audiences.
pub(super) fn emit_bonfire_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for bf in plan.bonfires() {
        let i = bf.index;
        let pos = bf.pos;
        // The three lines a `set-checkpoint` writes: vanilla's respawn point, the
        // `dw:cp` mirror every other feature reads, and the active-checkpoint
        // marker that selects the respawn hook. This IS "save".
        let save: Vec<String> = {
            let mut s = vec![
                format!("spawnpoint @a {} {} {}", pos[0], pos[1], pos[2]),
                format!(
                    "data modify storage dw:cp pos set value [{}, {}, {}]",
                    pos[0], pos[1], pos[2]
                ),
            ];
            if plan.any_checkpoint() {
                s.push(format!("scoreboard players set #cp dw.sys {i}"));
            }
            s
        };

        // --- the choice dialog opener (run AS the clicking player) ---
        // The advancement re-arms itself so a bonfire can be opened again and
        // again; `dw.rest` is reset before it is enabled so a stale answer from
        // an earlier rest can never fire the moment the dialog opens.
        fns.push((
            format!("bonfire_open_{i}"),
            lines(&[
                format!("advancement revoke @s only {ns}:bf_{i}"),
                format!("scoreboard players set @s dw.rest_at {i}"),
                "scoreboard players reset @s dw.rest".to_string(),
                "scoreboard players enable @s dw.rest".to_string(),
                format!("dialog show @s {ns}:bonfire_{i}"),
            ]),
        ));

        // --- option 1: save only. The checkpoint moves; NOTHING else happens. ---
        fns.push((format!("bonfire_save_{i}"), lines(&save)));
        fns.push((
            format!("bonfire_pick_save_{i}"),
            lines(&[
                "scoreboard players reset @s dw.rest".to_string(),
                format!("function {ns}:bonfire_save_{i}"),
            ]),
        ));

        // --- option 2: rest and save. Restore the resting player, then the
        // party-wide save + scene reset. ---
        fns.push((
            format!("bonfire_pick_rest_{i}"),
            lines(&[
                "scoreboard players reset @s dw.rest".to_string(),
                // A rest restores the WHOLE party, whoever sat down
                // (spec-0016 §1): every living player is healed, fed, cleansed,
                // mended and refilled. A body on its death screen is skipped; it
                // comes back at this fire with its flask refilled by the respawn.
                format!(
                    "execute as @a unless data entity @s {{Health:0.0f}} run function \
                     {ns}:bonfire_restore"
                ),
                format!("function {ns}:bonfire_rest_{i}"),
            ]),
        ));

        // `bonfire_rest_<i>` stays exactly what it was: the party-wide half of a
        // rest. The respawn path and the generated PackTests both drive it
        // directly, so it must remain callable with no player restore attached.
        let mut body = save;
        body.extend(bonfire_reseat_lines(plan));
        body.extend(emit_effect_bundle(
            plan,
            &bf.on_respawn,
            Audience::Scheduled,
        ));
        fns.push((format!("bonfire_rest_{i}"), lines(&body)));
    }
    if let Some(f) = emit_restore_function(plan) {
        fns.push(f);
    }
    if let Some(f) = emit_flask_function(plan) {
        fns.push(f);
    }
    fns
}

/// Whether the emitted setup must initialize the `dw:cp` last-checkpoint storage
/// mirror to the spawn cell. Single shared gate so the (idempotent) init line is
/// emitted exactly once regardless of merge order: a campaign needs it when it
/// declares a `set-checkpoint` (spec-0012 — the mirror must read before the first
/// checkpoint fires) OR a `boundary` (spec-0013 — its return clock reads the
/// mirror). Absent both, non-v0.6 output stays byte-identical.
pub(super) fn needs_cp_init(plan: &Plan) -> bool {
    !plan.checkpoints.is_empty() || plan.campaign.world.content.boundary.is_some()
}
