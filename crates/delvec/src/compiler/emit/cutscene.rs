//! Cutscenes: shots, the sneak-held predicate, repair, and the cutscene functions.

use super::*;

/// The generated function name for a `cutscene` effect, derived from its
/// **normalized shot list** — so the v0.4 single-shot spelling and a one-entry
/// `shots` list name the same function (byte-identical output).
///
/// Shape: `cs_<first anchor>_<first shot seconds>_<first shot waypoints>` — the
/// pre-multi-shot name — plus a `_<digest>` suffix over the whole shot list
/// (anchors, offsets, durations, subjects) whenever the cutscene is not a bare
/// single shot without `look_at`. The readable prefix keeps generated functions
/// greppable; the digest makes the key injective, so two cutscenes that share a
/// first waypoint but differ anywhere later can never collapse onto one function.
pub(super) fn cutscene_fn(
    shots: &[delvewright_dsl::CameraShot],
    party: delvewright_dsl::CutsceneParty,
) -> String {
    let name = cutscene_shots_fn(shots);
    match party {
        delvewright_dsl::CutsceneParty::Present => name,
        delvewright_dsl::CutsceneParty::Absent => format!("{name}_absent"),
    }
}

/// The shot-list half of [`cutscene_fn`]: the name a `present` cutscene keeps.
pub(super) fn cutscene_shots_fn(shots: &[delvewright_dsl::CameraShot]) -> String {
    let head = &shots[0];
    let first = head
        .path
        .first()
        .map(|w| plan::safe_local(w.anchor.as_str()))
        .or_else(|| {
            // A styled shot may have no explicit path: key on the style + the
            // subject's id instead, so the function name stays greppable.
            head.shot_style.map(|style| {
                let subj = match &head.subject {
                    Some(delvewright_dsl::CameraSubject::Anchor(s)) => s.anchor.as_str(),
                    Some(delvewright_dsl::CameraSubject::Npc(s)) => s.npc.as_str(),
                    Some(delvewright_dsl::CameraSubject::Actor(s)) => s.actor.as_str(),
                    None => "none",
                };
                format!(
                    "{}_{}",
                    plan::safe_local(style.token()),
                    plan::safe_local(subj)
                )
            })
        })
        .unwrap_or_else(|| "none".to_string());
    let base = format!("cs_{first}_{}_{}", head.resolved_seconds(), head.path.len());
    if shots.len() == 1 && head.look_at.is_none() && head.shot_style.is_none() {
        return base;
    }
    format!("{base}_{}", cutscene_digest(shots))
}

/// A short, stable content digest of a normalized cutscene shot list: the first
/// 8 hex chars of the sha256 of a canonical textual rendering. Deterministic
/// (fixed algorithm, fixed field order, no hash-order iteration, ADR-0006).
pub(super) fn cutscene_digest(shots: &[delvewright_dsl::CameraShot]) -> String {
    let mut canon = String::new();
    for shot in shots {
        canon.push_str(&format!("s={};", shot.resolved_seconds()));
        for w in &shot.path {
            canon.push_str(&format!(
                "p={}@{},{},{};",
                w.anchor.as_str(),
                w.offset[0],
                w.offset[1],
                w.offset[2]
            ));
        }
        if let Some(t) = &shot.look_at {
            canon.push_str(&format!(
                "l={}@{},{},{};",
                t.anchor.as_str(),
                t.offset[0],
                t.offset[1],
                t.offset[2]
            ));
        }
        // Styled-shot fields (v0.6, spec-0015) — appended only when present, so
        // every pre-existing shot list keeps its digest byte-for-byte.
        if let Some(style) = shot.shot_style {
            canon.push_str(&format!("y={};", style.token()));
        }
        if let Some(sub) = &shot.subject {
            canon.push_str(&format!("u={};", sub.canon()));
        }
        if let Some(sub) = &shot.subject_b {
            canon.push_str(&format!("v={};", sub.canon()));
        }
        if let Some(d) = shot.dist {
            canon.push_str(&format!("d={d:?};"));
        }
        if let Some(g) = shot.degrees {
            canon.push_str(&format!("g={g:?};"));
        }
        if let Some(b) = shot.bearing {
            canon.push_str(&format!("b={b:?};"));
        }
        canon.push('|');
    }
    sha256_hex(canon.as_bytes())[..8].to_string()
}

/// The entity tag every player carries for the duration of a cutscene.
///
/// **Staging invariant — a cutscene is pure observation.** While a player is in
/// the cutscene state, campaign machinery must not require anything of them and
/// must not punish them: the stealth judge is suspended for that player (grace
/// neither accrues nor expires, `on_caught` cannot fire) and `damage-players`
/// skips them. Any future verb that *demands* input or *deals harm* joins this
/// list — the player is watching, not playing.
///
/// Added by the cutscene `start` alongside `gamemode spectator`, removed by the
/// `end`/restore, so the state has exactly the cinematic's lifetime.
pub(crate) const CUTSCENE_TAG: &str = "dw_cutscene";

/// Datapack predicate id (under the campaign namespace) matching a player whose
/// sneak key is HELD this tick — the vanilla `minecraft:player` `input`
/// sub-predicate (1.21.2+), which reads the client's raw input packet and so
/// works in every gamemode, spectator included. Sole consumer: the cutscene
/// `spectate` bounce, which must not re-attach a player whose held sneak would
/// immediately dismount them again (the round-6 camera-flicker root cause).
/// Emitted only for a campaign with at least one cutscene, so everything else
/// stays byte-identical.
pub(super) const SNEAK_HELD_PREDICATE: &str = "sneak_held";

/// The `<ns>:sneak_held` predicate body (see [`SNEAK_HELD_PREDICATE`]).
pub(super) fn sneak_held_predicate() -> Value {
    json!({
        "condition": "minecraft:entity_properties",
        "entity": "this",
        "predicate": {
            "type_specific": {
                "type": "minecraft:player",
                "input": { "sneak": true }
            }
        }
    })
}

/// Does the campaign play at least one real cutscene (a non-empty shot list)?
/// Gates the [`SNEAK_HELD_PREDICATE`] emission.
pub(super) fn campaign_has_cutscene(campaign: &delvewright_dsl::Campaign) -> bool {
    crate::compiler::camera::cutscene_units(campaign)
        .iter()
        .any(|(eff, _)| eff.cutscene_shots().is_some_and(|s| !s.is_empty()))
}

/// The campaign-wide count of cutscenes currently playing. A refcount rather than
/// a flag because nothing forbids two cutscenes overlapping (each `cs_<bare>` only
/// guards re-entry into *itself*). Never initialized, so an `unless … matches 1..`
/// test reads correctly before the first cutscene ever runs.
pub(super) const CS_LIVE: &str = "#cs_live";

/// The `tick` clause that repairs a player stranded by a mid-cutscene disconnect.
///
/// The cutscene bracket is entirely `@a`-scoped: `cs_end_<bare>` restores gamemode,
/// teleports, and removes [`CUTSCENE_TAG`] from *the players who are online when it
/// ends*. A player who dropped during the shot is not among them, so they come back
/// tagged, in spectator, with the marker they would have been teleported to already
/// killed. `join_place` is no help — it is gated on `dw_joined`, which survives a
/// relog exactly like the cutscene tag does.
///
/// The stuck state is decidable without any per-player bookkeeping: *tagged
/// `dw_cutscene` while no cutscene is playing*. Empty for a cutscene-less campaign,
/// so those packs stay byte-identical.
pub(super) fn cutscene_repair_tick(plan: &Plan) -> Vec<String> {
    if !campaign_has_cutscene(plan.campaign) {
        return Vec::new();
    }
    let ns = &plan.namespace;
    // A player waiting out a respawn (spec-0077) carries the tag outside any
    // cutscene on purpose; the wait releases them, not this repair.
    let waiting = if respawn_wait(plan).is_some() {
        format!("unless score @s {RW_CLOCK} matches 1.. ")
    } else {
        String::new()
    };
    vec![format!(
        "execute unless score {CS_LIVE} dw.sys matches 1.. as @a[tag={CUTSCENE_TAG}] {waiting}run function {ns}:cs_repair"
    )]
}

/// The repair itself, per stranded player (`@s`): back to adventure, untagged, and
/// returned to the party's live checkpoint.
///
/// The destination is `storage dw:cp pos` — the checkpoint mirror, seeded to the
/// entry point at setup and rewritten by every `set-checkpoint` — because the
/// cutscene's own saved position (a single `dw_csmark_<bare>` marker) is destroyed
/// by `cs_end_<bare>` before this can ever run. It is a macro teleport for the same
/// reason the boundary return is one: the mirror is an `[x, y, z]` list, not
/// tp-shaped arguments. Emitted only when the campaign resolves an entry anchor,
/// which is what seeds the mirror in the first place.
pub(super) fn cutscene_repair_fns(plan: &Plan) -> Vec<(String, String)> {
    if !campaign_has_cutscene(plan.campaign) {
        return Vec::new();
    }
    let ns = &plan.namespace;
    let mut repair = vec![
        "gamemode adventure @s".to_string(),
        format!("tag @s remove {CUTSCENE_TAG}"),
    ];
    let mut out = Vec::new();
    if campaign_spawn(plan).is_some() {
        for (i, axis) in ["x", "y", "z"].iter().enumerate() {
            repair.push(format!(
                "data modify storage dw:cs at.{axis} set from storage dw:cp pos[{i}]"
            ));
        }
        repair.push(format!("function {ns}:cs_repair_tp with storage dw:cs at"));
        out.push((
            "cs_repair_tp".to_string(),
            lines(&["$tp @s $(x) $(y) $(z)".to_string()]),
        ));
    }
    out.push(("cs_repair".to_string(), lines(&repair)));
    out
}

/// Cutscene functions (spec-0008 addendum; keyframe dolly): the
/// two-camera bounce. Per cutscene (deduped by content key) emits a start
/// function, a self-scheduling keyframe/`spectate` driver, and an end/restore
/// function.
///
/// Mechanic: save each player's return point (a marker at a representative
/// player), spectator, then dolly two co-located invisible cameras along the
/// shot's keyframe schedule — a `tp` every `cadence` ticks with display-entity
/// `teleport_duration` set to the cadence, so the *client* tweens position and
/// rotation between keyframes ([`crate::compiler::camera`], spike-measured) — while
/// alternating `spectate` between the pair each tick (the naive same-entity
/// re-`spectate` is a server no-op — never emitted; the bounce cannot reset an
/// in-flight tween, measurement 4). The bounce skips any player actively
/// holding sneak (`predicate=!<ns>:sneak_held`, see [`SNEAK_HELD_PREDICATE`]):
/// sneak dismounts a spectator, so re-attaching against a held key strobes.
/// On completion, restore adventure mode + teleport players back to the marker.
///
/// **Path timing**: the dolly is arc-length parameterized (equal
/// distance per time, not equal segments per time) with baked smoothstep
/// ease-in/ease-out — both fixes live in [`crate::compiler::camera::plan_shot`].
///
/// **Aim** (DSL v0.6): every dolly `tp` carries an explicit `<yaw> <pitch>`, so a
/// spectating player looks where the shot means them to look instead of at the
/// summon default (yaw 0 = south). With `look_at`, the rotation is computed per
/// keyframe from the camera's own position toward the subject point (the framing
/// holds through the whole move, with the client tweening rotation between
/// keyframes); without it, the camera faces along the eased path's direction of
/// travel. Pure `atan2` on plan coordinates, rounded to 3 decimals:
/// deterministic, no RNG, no wall clock.
///
/// **Multi-shot** (DSL v0.6): a cutscene is a list of shots played back-to-back
/// inside ONE save/restore bracket — one marker, one `gamemode spectator`, one
/// camera pair, one restore. The shots share the single `#t_<bare>` tick counter:
/// shot `k` owns the half-open-on-the-right window `[offset_k, offset_k + len_k]`
/// and the next shot starts at `offset_k + len_k + 1`, so the transition is a hard
/// cut (the next tick teleports the camera pair to the new shot's first waypoint
/// with its own aim). A one-shot cutscene reduces to exactly the pre-multi-shot
/// timeline, so the single-shot spelling is byte-identical either way.
pub(super) fn cutscene_fns(
    plan: &Plan,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (eff, ctx) in crate::compiler::camera::cutscene_units(plan.campaign) {
        let Some(shots) = eff.cutscene_shots().filter(|s| !s.is_empty()) else {
            continue;
        };
        // `start` = the function emit_quest_effect calls (`cs_<bare>`); `bare` is
        // the shared suffix for the tick/end functions and per-cutscene sentinels.
        // Dedup is by DSL content: two byte-identical cutscene effects share one
        // generated function, planned from the FIRST occurrence's move context
        // (deterministic — the traversal order is fixed). An author who wants a
        // styled moving-subject cutscene to differ per context gives the shots
        // distinguishing content (e.g. an explicit `seconds`).
        let party = eff.cutscene_party().unwrap_or_default();
        let start_name = cutscene_fn(&shots, party);
        if !seen.insert(start_name.clone()) {
            continue;
        }
        let bare = start_name
            .strip_prefix("cs_")
            .unwrap_or(&start_name)
            .to_string();
        // Expand every shot (explicit path, or `shot_style` construction) to
        // its resolved geometry + aim. The air-corridor / chord / angular
        // checks (crate::compiler::nav, DW0308/DW0347) validate these exact expansions.
        let resolved: Vec<crate::compiler::camera::ExpandedShot> = {
            let mut off: i32 = 0;
            shots
                .iter()
                .map(|shot| {
                    let ex = crate::compiler::camera::expand_shot(
                        plan,
                        moves,
                        actor_moves,
                        shot,
                        &ctx,
                        off,
                    );
                    off += ex.ticks + 1;
                    ex
                })
                .collect()
        };
        let first =
            resolved[0]
                .clip_polyline()
                .first()
                .copied()
                .unwrap_or([0.0, plan::BASE_Y as f64, 0.0]);

        // start
        let mut start: Vec<String> = Vec::new();
        start.push(format!(
            "execute if score #run_{bare} dw.sys matches 1 run return fail"
        ));
        start.push(format!("scoreboard players set #run_{bare} dw.sys 1"));
        // The campaign-wide "some cutscene is playing" refcount. Placed AFTER the
        // re-entry `return fail` so a re-entrant start never inflates it. Read by
        // the join repair driver — see `cutscene_repair_fns`.
        start.push(format!("scoreboard players add {CS_LIVE} dw.sys 1"));
        start.push(format!("scoreboard players set #t_{bare} dw.sys 0"));
        start.push(format!("scoreboard players set #p_{bare} dw.sys 1"));
        // The return point is a player in play: a waiting player (spec-0077)
        // already carries the observation tag and may be anywhere.
        let marker_at = match observer_guard(plan).strip_prefix(',') {
            Some(g) => format!("@p[{g}]"),
            None => "@p".to_string(),
        };
        start.push(format!(
            "execute at {marker_at} run summon minecraft:marker ~ ~ ~ {{Tags:[{FIXTURE_NBT}\"dw_csmark_{bare}\"]}}"
        ));
        // spec-0095: the party stays in the scene. Each player in play leaves a
        // stand-in where they stand, summoned from the body before spectator
        // takes it out of the world (`crate::compiler::standin`).
        if party == delvewright_dsl::CutsceneParty::Present {
            start.extend(crate::compiler::standin::start_lines(
                ns,
                &bare,
                CUTSCENE_TAG,
            ));
        }
        // The cutscene state marker. `gamemode spectator` already takes the
        // players' bodies out of the world; the tag is what campaign machinery
        // reads so it does not keep asking anything of a player who is only
        // watching (see CUTSCENE_TAG).
        start.push(format!("tag @a add {CUTSCENE_TAG}"));
        start.push("gamemode spectator @a".to_string());
        for cam in ["a", "b"] {
            start.push(format!(
                "summon minecraft:item_display {} {} {} {{Tags:[\"dw_cam_{bare}\",\"dw_cam{cam}_{bare}\"]}}",
                fmt_f64(first[0]), fmt_f64(first[1]), fmt_f64(first[2])
            ));
        }
        start.push(format!("schedule function {ns}:cs_tick_{bare} 1t"));
        out.push((start_name.clone(), lines(&start)));

        // Keyframe driver: every shot's keyframes laid end-to-end on
        // one counter. Each shot plans an arc-length-parameterized, eased
        // keyframe schedule (`crate::compiler::camera::plan_shot`); the client draws the
        // in-between frames via display-entity `teleport_duration` (= the
        // shot's cadence), tweening position AND rotation — see the spike
        // measurements in `crate::compiler::camera`'s module docs.
        let mut tick: Vec<String> = Vec::new();
        let mut offset: i32 = 0;
        for (si, shot) in resolved.iter().enumerate() {
            let sf = shot.frames();
            // Cadence merge + snap share the shot's first tick: the position
            // sync is flushed before entity metadata within a tick (spike
            // measurement 5), so the snap `tp` lands instantly under the OLD
            // duration (0 — the summon default, or the previous shot's reset)
            // and the new cadence governs only the keyframes that follow.
            if sf.cadence > 0 {
                tick.push(format!(
                    "execute if score #t_{bare} dw.sys matches {offset} as @e[tag=dw_cam_{bare}] run data merge entity @s {{teleport_duration:{}}}",
                    sf.cadence
                ));
            }
            for f in &sf.frames {
                tick.push(format!(
                    "execute if score #t_{bare} dw.sys matches {} run tp @e[tag=dw_cam_{bare}] {} {} {} {} {}",
                    offset + f.tick,
                    fmt_f64(f.pos[0]), fmt_f64(f.pos[1]), fmt_f64(f.pos[2]),
                    fmt_f64(f.yaw), fmt_f64(f.pitch)
                ));
            }
            // Re-arm the hard cut: reset `teleport_duration` on the shot's last
            // owned tick — no keyframe is issued then, and a metadata change
            // does not disturb an in-flight tween (measurement 4/5) — so the
            // NEXT shot's snap is instant, not a glide.
            if sf.cadence > 0 && si + 1 < resolved.len() {
                tick.push(format!(
                    "execute if score #t_{bare} dw.sys matches {} as @e[tag=dw_cam_{bare}] run data merge entity @s {{teleport_duration:0}}",
                    offset + shot.ticks
                ));
            }
            offset += shot.ticks + 1;
        }
        // The last frame emitted sits at `offset - 1`; the driver ends one tick later.
        let total: i32 = offset - 1;
        // alternate `spectate` between the two co-located cameras (the bounce):
        // parity 1 → camera a, parity 2 → camera b, flipped each tick — but
        // NEVER at a player actively holding sneak. In spectator mode the sneak
        // key dismounts the spectated entity, so an unconditional per-tick
        // re-attach strobes (attach → client dismount → attach …) for as long
        // as the key is held (round-6 owner report). The vanilla `input` player
        // predicate ([`SNEAK_HELD_PREDICATE`], 1.21.2+) reads the raw key
        // state — including in spectator — so a held sneak yields a stable
        // detached spectator (frozen, staring at the world) and release
        // re-attaches on the next bounce tick, resuming the shot.
        tick.push(format!(
            "execute if score #p_{bare} dw.sys matches 1 as @a[predicate=!{ns}:{SNEAK_HELD_PREDICATE}] run spectate @n[type=minecraft:item_display,tag=dw_cama_{bare}] @s"
        ));
        tick.push(format!(
            "execute if score #p_{bare} dw.sys matches 2 as @a[predicate=!{ns}:{SNEAK_HELD_PREDICATE}] run spectate @n[type=minecraft:item_display,tag=dw_camb_{bare}] @s"
        ));
        tick.push(format!(
            "execute if score #p_{bare} dw.sys matches 2 run scoreboard players set #p_{bare} dw.sys 1"
        ));
        tick.push(format!(
            "execute if score #p_{bare} dw.sys matches 1 run scoreboard players set #p_{bare} dw.sys 2"
        ));
        tick.push(format!("scoreboard players add #t_{bare} dw.sys 1"));
        tick.push(format!(
            "execute if score #t_{bare} dw.sys matches {}.. run function {ns}:cs_end_{bare}",
            total + 1
        ));
        tick.push(format!(
            "execute unless score #t_{bare} dw.sys matches {}.. run schedule function {ns}:cs_tick_{bare} 1t",
            total + 1
        ));
        out.push((format!("cs_tick_{bare}"), lines(&tick)));

        // end / restore: leaving spectator returns each player to their
        // pre-spectator position; the explicit tp to the saved marker makes the
        // restore robust (spec addendum: restore gamemode + position).
        let mut end: Vec<String> = vec![
            "gamemode adventure @a".to_string(),
            format!("tp @a @e[tag=dw_csmark_{bare},limit=1]"),
            format!("kill @e[tag=dw_cam_{bare}]"),
            format!("kill @e[tag=dw_csmark_{bare}]"),
        ];
        // spec-0095: the stand-ins leave as the players return, by the engine's
        // one unseen removal (no death animation where the player now stands).
        if party == delvewright_dsl::CutsceneParty::Present {
            end.extend(removal_lines(
                ns,
                &crate::compiler::standin::cutscene_tag(&bare),
                false,
                Exit::Unseen,
            ));
        }
        // Resume: drop the cutscene marker. The stealth judge (zone-presence
        // only — no sneak stat is tracked) needs no re-sync;
        // grace is deliberately NOT reset — it neither accrued nor expired
        // during the cutscene, so the beat picks up exactly where it paused.
        end.push(format!("tag @a remove {CUTSCENE_TAG}"));
        end.push(format!("scoreboard players set #run_{bare} dw.sys 0"));
        end.push(format!("scoreboard players remove {CS_LIVE} dw.sys 1"));
        out.push((format!("cs_end_{bare}"), lines(&end)));
    }
    if cutscene_parties(plan)
        .iter()
        .any(|(_, p)| *p == delvewright_dsl::CutsceneParty::Present)
    {
        out.push((
            crate::compiler::standin::STANDIN_FN.to_string(),
            lines(&crate::compiler::standin::standin_fn_body(ns)),
        ));
    }
    out
}

/// Every cutscene's start function name with its declared party (spec-0095),
/// deduplicated exactly as [`cutscene_fns`] deduplicates them.
pub(super) fn cutscene_parties(plan: &Plan) -> Vec<(String, delvewright_dsl::CutsceneParty)> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for (eff, _) in crate::compiler::camera::cutscene_units(plan.campaign) {
        let Some(shots) = eff.cutscene_shots().filter(|s| !s.is_empty()) else {
            continue;
        };
        let party = eff.cutscene_party().unwrap_or_default();
        let name = cutscene_fn(&shots, party);
        if seen.insert(name.clone()) {
            out.push((name, party));
        }
    }
    out
}
