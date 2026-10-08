//! Effect bundles: audience and command source, and the lowering of every quest effect.

use super::*;

/// Fail the build if any campaign effect — at **every effect root**, at **any
/// nesting depth** — names an anchor that resolves to no world position
/// (`DW0360`). This is the single resolved-anchor-or-diagnostic seal over the
/// whole anchor-bearing effect surface ([`QuestEffect::anchor_refs`], the
/// nesting-aware sibling of [`QuestEffect::nested_effect_lists`]).
///
/// It exists because every anchor consumer in [`emit_quest_effect`] fails *open*:
/// `open-gate`/`close-gate` scan `plan.anchors` for a name match and simply fall
/// out of the loop, `set-block`/`set-checkpoint`/`play-sound`/`damage-players`
/// bail out of an `if let Some(pos)`, and a cutscene waypoint silently degrades to
/// `[0, BASE_Y, 0]`. A single typo'd anchor therefore used to emit **nothing** —
/// a gate that never opens, a checkpoint that never binds — and shipped a broken
/// delve into the owner's one QA hour. `DW0142` catches what it can at DSL time,
/// but it only sees an area's declared anchor set (pool areas and cross-area
/// camera anchors are deferred to here), so this is the backstop that makes the
/// rule total.
///
/// **Total means total**. The roots come from
/// [`crate::compiler::plan::for_each_effect_root`] — the one enumeration
/// [`all_campaign_effects`], the staged-walk timeline and both halves of
/// `compiler::flow` also walk. This walk used to hand-list three of the five, so a
/// typo'd anchor in a `traps[].payload` or a dialogue option's `set-checkpoint`
/// `on_respawn` bundle was never asked the question at all: the build stayed
/// green, `trap_fire_<trap>.mcfunction` shipped with the `open-gate` simply
/// absent, and the delve the owner played had a trap that springs and does
/// nothing. A backstop that reaches four fifths of the surface is exactly the
/// silent-drop class it was written to end, so the roots are now inherited rather
/// than re-listed and a sixth root cannot be forgotten here.
pub(super) fn check_effect_anchors(plan: &Plan) -> Result<(), BuildFailure> {
    let c = plan.campaign;
    // (json pointer, effect verb, anchor) for every anchor reference in the
    // campaign, deep, in deterministic content order.
    let mut refs: Vec<(String, &'static str, String)> = Vec::new();
    // This seal covers the verbs that fail OPEN — the ones whose anchor consumer
    // shrugs and emits nothing. The spec-0022 payload verbs (`volley`,
    // `collapse`) fail CLOSED instead: `plan_payload_verbs` resolves their volumes
    // with `?` and reports `DW0447`, naming the verb, the volume and the anchor.
    // Widening this walk to R4/R5 put those anchors in reach of the
    // generic message for the first time, which would have preempted the specific
    // one for no gain — so where `DW0447` runs, the fail-closed verbs keep it.
    //
    // **Only where it runs.** `plan_payload_verbs` lives inside the world block,
    // so it is reached only when the campaign assembles a world. A payload verb
    // does NOT imply that: nothing confines `volley`/`collapse` to
    // `traps[].payload` — `dsl::validate` reaches them via
    // `for_each_trap_payload_deep` inside the traps loop, which validates them
    // where they are rather than forbidding them elsewhere, and they are ordinary
    // variants of the shared effect enum. A `volley` on a quest's `on_complete` in
    // a campaign with no traps, no waves, no bodies and no walkable critical leg
    // therefore reaches emission with `DW0447` unreachable. Deferring there would
    // trade a specific message for SILENCE, so the deferral is conditional on the
    // proof actually running and this seal keeps that corner itself.
    let payload_verbs_are_proven = assembles_world(plan);
    fn descend(
        path: String,
        eff: &QuestEffect,
        payload_verbs_are_proven: bool,
        refs: &mut Vec<(String, &'static str, String)>,
    ) {
        let defer_to_dw0447 =
            payload_verbs_are_proven && (eff.volley().is_some() || eff.collapse().is_some());
        if !defer_to_dw0447 {
            // The demanded shape is the DSL tier's business (`DW0871`); this
            // seal is about whether the name resolves at all.
            for (suffix, anchor, _kind) in eff.anchor_refs() {
                refs.push((
                    format!("{path}/{suffix}"),
                    eff.verb.tag(),
                    anchor.as_str().to_string(),
                ));
            }
        }
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(
                    format!("{path}/{pseg}/{j}"),
                    inner,
                    payload_verbs_are_proven,
                    refs,
                );
            }
        }
    }
    crate::compiler::plan::for_each_effect_root(c, &mut |site, effs| {
        for (i, eff) in effs.iter().enumerate() {
            descend(
                format!("{}/{i}", site.path),
                eff,
                payload_verbs_are_proven,
                &mut refs,
            );
        }
    });
    // spec-0082: an assembly's mark and its arming region are anchor-bearing
    // declarations like every other, and an unresolved one would place nothing
    // and judge nothing.
    for (i, a) in c.quests.content.assemblies.iter().enumerate() {
        refs.push((
            format!("/content/assemblies/{i}/at/anchor"),
            "assembly",
            a.at.anchor.as_str().to_string(),
        ));
        if let Some(st) = &a.strikes {
            refs.push((
                format!("/content/assemblies/{i}/strikes/while_in/anchor"),
                "assembly",
                st.while_in.anchor.as_str().to_string(),
            ));
            for (j, step) in st.pattern.iter().enumerate() {
                if let Some(l) = &step.lock {
                    refs.push((
                        format!("/content/assemblies/{i}/strikes/pattern/{j}/lock/within/anchor"),
                        "assembly",
                        l.within.anchor.as_str().to_string(),
                    ));
                }
            }
        }
    }
    for (path, verb, anchor) in refs {
        if anchor_point_any(plan, &anchor).is_some() {
            continue;
        }
        return Err(BuildFailure::Diagnostic {
            code: DW_EFFECT_ANCHOR_UNRESOLVED,
            message: format!(
                "`{verb}` at `{path}` names anchor `{anchor}`, which resolves to no \
                 position in the assembled world — the effect would emit nothing at \
                 all (a gate that never opens, a block never placed, a camera stuck \
                 at the world origin) — {}",
                delvewright_dsl::Placement::of(c).anchor_remedy(
                    "anchor names come from prefab metadata: use one the area's prefab/pool \
                     actually exposes, and do NOT invent one"
                ),
            ),
        });
    }
    Ok(())
}

/// Whether every site that fires `spawn-wave` for `wave_id` is a root with no
/// area of its own — a trigger, a trap payload, an actor's `on_kill`, a shop
/// offer, a shortcut's unlock, `on_death`, an assembly's landing or a loop's
/// crossing — so that in a campaign of several areas nothing says where the wave
/// forms up. Read for `DW0310`'s message, so the refusal names the shape.
pub(super) fn fired_only_by_global_roots(c: &delvewright_dsl::Campaign, wave_id: &str) -> bool {
    let mut any = false;
    let mut all_global = true;
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        let mut fires = false;
        for e in list {
            e.visit_deep(&mut |x| {
                if matches!(x.spawn_wave(), Some(w) if w.as_str() == wave_id) {
                    fires = true;
                }
            });
        }
        if fires {
            any = true;
            if matches!(
                site.owner,
                delvewright_dsl::EffectRootOwner::ObjectiveComplete { .. }
                    | delvewright_dsl::EffectRootOwner::QuestComplete { .. }
            ) {
                all_global = false;
            }
        }
    });
    any && all_global
}

/// Who an emitted effect bundle speaks to, and whether it has an acting player.
///
/// **Party state (spec-0018).** Objective/quest/flag progression lives on the
/// [`plan::PARTY`] holder, so a *party-fact* effect (`set-flag`, `open-gate`,
/// `spawn-*`, `set-checkpoint`, a driver start, …) names no player at all and
/// fires exactly once, under every audience. Only *player-facing* effects
/// (`narrate`, `play-sound`, `damage-players`, `give-item`) need a selector, and
/// that selector is what this enum decides.
///
/// **The scheduled-executor bug this still models (AUDIT-P0).** Vanilla's
/// `schedule function …` re-invokes a function with the **server** command
/// source: no executor, so `@s` resolves to nothing and every `@s`-addressed
/// command silently fails. Under party state a scheduled `set-flag` writes
/// `#party` and is immune by construction; what remains executor-dependent is
/// exactly one thing — a `carrier: "one"` `give-item`, which needs the acting
/// player — and [`Audience::Scheduled`] is where that has no answer (rejected at
/// validate time, `DW0357`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Audience {
    /// A **party event** entered as one player (`complete_<obj>`, `complete_q_*`,
    /// `trig_<id>`): player-facing effects address `@a` (the whole party sees the
    /// beat), and `@s` is available as the completing player for a `carrier:
    /// "one"` hand-off.
    Party,
    /// A **scheduled** bundle (`mv_arrive_*`, `ma_arrive_*`, `seq_*_<i>`), entered
    /// with the server command source: player-facing effects address `@a`; there
    /// is no `@s` at all.
    Scheduled,
    /// **One player's own** bundle: a checkpoint `on_respawn` (fired per
    /// respawning player) and a stealth `on_caught` (fired at the spotted
    /// player). Player-facing effects address `@s` — re-broadcasting one player's
    /// death or exposure to the party would duplicate their kit and their
    /// narration.
    Solo,
}

impl Audience {
    /// The selector a player-facing command uses.
    fn selector(self) -> &'static str {
        match self {
            Audience::Party | Audience::Scheduled => "@a",
            Audience::Solo => "@s",
        }
    }

    /// Is there an acting player (`@s`) in this command source?
    pub(super) fn has_actor(self) -> bool {
        !matches!(self, Audience::Scheduled)
    }
}

/// **The audience each effect root's bundle is emitted under.**
///
/// One function rather than seven literals at seven call sites, because
/// `EffectRootKind::runs_with_acting_player` states the same fact for validation
/// (`DW0503`) and the two must not drift: a root whose emitted bundle quietly
/// changed audience would turn a validated `player`-scoped read into an `@s` in
/// a sourceless function, with every check green. `root_audience_matches_the_dsl`
/// binds them by equality over the closed root set.
///
/// Byte impact: none. Each arm is the literal the call site already passed.
pub(super) fn root_audience(kind: delvewright_dsl::EffectRootKind) -> Audience {
    use delvewright_dsl::EffectRootKind as K;
    match kind {
        // Dispatched `as @a` from the tick: the acting player is `@s`.
        K::ObjectiveComplete | K::QuestComplete => Audience::Party,
        // The dying / respawning player's own beat.
        K::DialogueRespawn | K::OnDeath => Audience::Solo,
        // The buying player's own beat: the handler is dispatched
        // `as @a[scores={…}]`, so `@s` is whoever pressed the button.
        K::ShopOffer => Audience::Solo,
        // The credited killer's own beat (spec-0074): the kill advancement's
        // reward runs as the player vanilla credited, so one kill is not
        // narrated to the whole party.
        K::OnKill => Audience::Solo,
        // Polled on the tick with no executor.
        K::Trigger | K::TrapPayload | K::ShortcutUnlock => Audience::Scheduled,
        // A blow lands from the per-assembly strike machine on the tick, with
        // no executor (spec-0082 §4.3).
        K::AssemblyLand => Audience::Scheduled,
        // A loop's answer (spec-0086 §3.5) is the dungeon acting, run from the
        // server source after the move: no executor, like a trap's payload.
        K::LoopCross => Audience::Scheduled,
    }
}

/// Emit a quest effect, wrapping every command it produces in the effect's
/// **party** flag guard when it declares `requires_flags` and/or `forbids_flags`
/// (DSL v0.6). Flags are party state (spec-0018), so the guard is one form under
/// every audience: `execute if score #party dw.f_<flag> matches 1 [… per required
/// flag] unless score #party dw.f_<flag> matches 1 [… per forbidden flag] run
/// <command>`. `unless … matches 1` deliberately treats an **unset** score as
/// "not set" (flag scores are never pre-initialized to 0, so a `scores={…=..0}`
/// selector would not work). An ungated effect (both lists empty) is emitted
/// verbatim.
pub(super) fn emit_gated_effect(
    plan: &Plan,
    eff: &QuestEffect,
    aud: Audience,
    body: &mut Vec<String>,
) {
    let mut inner: Vec<String> = Vec::new();
    emit_quest_effect(plan, eff, aud, &mut inner);
    guard_effect_lines(plan, eff, inner, body);
}

/// Wrap lines lowered for `eff` in its own gate (`when`), the one guard every
/// gated effect takes — also for lines a feature module lowered itself (an
/// aimed assembly's turned blow, spec-0082 §5.5).
pub(crate) fn guard_effect_lines(
    plan: &Plan,
    eff: &QuestEffect,
    inner: Vec<String>,
    body: &mut Vec<String>,
) {
    let gate = eff.gate();
    if gate.is_empty() {
        body.extend(inner);
        return;
    }
    // DSL v0.10: the numeric terms join the flag terms in one guard, in gate
    // field order — `Plan::gate_terms`, the same reduction the death plan hands
    // the bot tier. The clauses come back unspaced; this guard is built in the
    // space-TERMINATED form, so each is re-spaced rather than concatenated
    // verbatim.
    let guard: String = plan
        .gate_terms(gate)
        .iter()
        .map(|t| format!("{} ", t.clause(false)))
        .collect();
    for line in inner {
        body.push(with_execute_prefix(&guard, line));
    }
}

/// Splice an `execute` prefix (already space-terminated, e.g. `if score #party
/// dw.f_x matches 1 `) onto one emitted command, folding into a leading
/// `execute` when there is one rather than nesting a second `execute … run
/// execute …`.
pub(super) fn with_execute_prefix(prefix: &str, line: String) -> String {
    if prefix.is_empty() {
        return line;
    }
    match line.strip_prefix("execute ") {
        Some(rest) => format!("execute {prefix}{rest}"),
        None => format!("execute {prefix}run {line}"),
    }
}

/// Emit a whole effect bundle for `aud` (see [`Audience`]).
pub(super) fn emit_effect_bundle<'a>(
    plan: &Plan,
    effects: impl IntoIterator<Item = &'a QuestEffect>,
    aud: Audience,
) -> Vec<String> {
    let mut body: Vec<String> = Vec::new();
    for e in effects {
        emit_gated_effect(plan, e, aud, &mut body);
    }
    body
}

/// **The one runtime region write** (DSL v0.10, spec-0031): fill the inclusive box
/// `region` with `block`, optionally restricted to the cells currently holding
/// `only`.
///
/// Every verb that writes a region at runtime goes through here — `fill-region`
/// (author's box, author's block), `clear-region` (author's box, air),
/// `close-gate` (the gate anchor's box and its declared block), `open-gate`
/// (the gate anchor's box, air, `replace`-filtered to the gate block so an opened
/// threshold never scrubs anything that drifted into it) and `open-way` (a placed
/// piece's exported way: its own cells, its own block for a `laid` one and air for
/// a `cleared` one). The `replace` filter is the only difference between them,
/// which is why it is a parameter here rather than five spellings of `fill` in
/// five match arms.
pub(super) fn fill_region_command(
    region: ([i32; 3], [i32; 3]),
    block: &str,
    only: Option<&str>,
) -> String {
    let (from, to) = region;
    let filter = match only {
        Some(o) => format!(" replace {o}"),
        None => String::new(),
    };
    format!(
        "fill {} {} {} {} {} {} {block}{filter}",
        from[0], from[1], from[2], to[0], to[1], to[2]
    )
}

/// The block a cleared region is written with. Named because three verbs share it.
pub(super) const AIR: &str = "minecraft:air";

/// **The audience one effect is emitted under** (spec-0085 §3.2): the
/// envelope's `audience` where it states one, else the bundle's own.
///
/// `actor` is `@s`, so it maps to [`Audience::Solo`]; `party` is `@a`, which
/// keeps the bundle's acting player where it has one ([`Audience::Party`]) and
/// stays [`Audience::Scheduled`] where it has none. An `actor` where the bundle
/// has no acting player is refused at validation (`DW0503`) and emitted under
/// the bundle's own audience rather than as an `@s` with nobody behind it.
///
/// Byte impact: none for an effect that states no `audience`.
pub(super) fn effect_audience(eff: &QuestEffect, aud: Audience) -> Audience {
    use delvewright_dsl::EffectAudience;
    match eff.audience {
        None => aud,
        Some(EffectAudience::Actor) if aud.has_actor() => Audience::Solo,
        Some(EffectAudience::Actor) => aud,
        Some(EffectAudience::Party) if aud.has_actor() => Audience::Party,
        Some(EffectAudience::Party) => Audience::Scheduled,
    }
}

/// **One effect's emitted commands, under a party or a solo bundle** — the
/// instrument `crates/delvec/tests/v35_perception.rs` binds
/// [`delvewright_dsl::Verb::addresses_players`] to the emitter with: a verb's
/// commands differ between the two exactly when it addresses players.
///
/// `solo` emits under [`Audience::Solo`] (`@s`), else [`Audience::Party`]
/// (`@a`); both have an acting player, so the only thing that moves is the
/// selector.
pub fn effect_commands(plan: &Plan, eff: &QuestEffect, solo: bool) -> Vec<String> {
    let aud = if solo {
        Audience::Solo
    } else {
        Audience::Party
    };
    let mut body = Vec::new();
    emit_quest_effect(plan, eff, aud, &mut body);
    body
}

/// Emit a quest effect's commands into `body`, addressing `aud`.
///
/// The envelope's `audience` and `in` (spec-0085 §3.2) are resolved here, once,
/// for every verb the emitter addresses to players: `who` is the audience's
/// selector narrowed by the `in` box. `damage-players` takes the box apart from
/// the selector, because its own filter carries the cutscene guard beside it.
pub(super) fn emit_quest_effect(
    plan: &Plan,
    eff: &QuestEffect,
    aud: Audience,
    body: &mut Vec<String>,
) {
    let ns = &plan.namespace;
    let aud = effect_audience(eff, aud);
    let narrowed: String = match eff.within.as_ref() {
        Some(zone)
            if eff.addresses_players() && !matches!(eff.verb, Verb::DamagePlayers { .. }) =>
        {
            // An unresolved box is `DW0142` at validation; emitting a selector with
            // a blank box would be an invalid command rather than a diagnosis.
            match effect_selector(plan, aud.selector(), Some(zone)) {
                // A box is the bodies standing in it. A player watching a
                // cutscene is a spectator whose camera may stand anywhere, so the
                // box excludes the observation tag (`DW0926`) — save for a status
                // effect, which `observer::ALLOWED` names as asking nothing of a
                // watcher.
                Some(sel)
                    if !matches!(eff.verb, Verb::GiveEffect { .. } | Verb::ClearEffect { .. }) =>
                {
                    unwatched(sel)
                }
                Some(sel) => sel,
                None => return,
            }
        }
        _ => aud.selector().to_string(),
    };
    let who = narrowed.as_str();
    match &eff.verb {
        Verb::OpenGate { anchor, .. } => {
            // Find the gate anchor across areas (first match).
            for ((_, name), resolved) in &plan.anchors {
                if name == anchor.as_str()
                    && let ResolvedAnchor::Gate { from, to, block } = resolved
                {
                    // A region write whose box and filter the gate anchor supplies.
                    body.push(fill_region_command((*from, *to), AIR, Some(block)));
                    // …and take the seal's answer down with the seal.
                    // The hitboxes exist exactly while the region is solid: an
                    // opened threshold that still says "the way is sealed" is a
                    // lie, and an invisible box left standing in a doorway
                    // swallows right-clicks aimed through it.
                    if let Some(s) = seal_hint_for(plan, anchor.as_str()) {
                        body.push(format!("kill @e[tag=dw_seal_{}]", s.safe));
                    }
                    return;
                }
            }
        }
        Verb::CloseGate { anchor, .. } => {
            // The physical dual of `open-gate`: fill the gate region with the block
            // the anchor declares (basalt boulder, iron bars, …), sealing it back
            // into a wall. A blockless gate anchor is rejected at validate-time
            // (`DW0343`), so the resolved `block` is the real fill here.
            for ((_, name), resolved) in &plan.anchors {
                if name == anchor.as_str()
                    && let ResolvedAnchor::Gate { from, to, block } = resolved
                {
                    // The same region write, with the block the anchor declares.
                    body.push(fill_region_command((*from, *to), block, None));
                    // Arm the seal's answer:
                    // a wall the party walks back to and presses must say
                    // something. Guarded on absence, so a re-fired `close-gate`
                    // never stacks a second set of hitboxes.
                    if let Some(s) = seal_hint_for(plan, anchor.as_str()) {
                        body.push(format!(
                            "execute unless entity @e[tag=dw_seal_{}] run function {ns}:{}",
                            s.safe,
                            seal_arm_fn(&s.safe)
                        ));
                    }
                    return;
                }
            }
        }
        Verb::CampaignComplete { .. } => {
            body.push(format!("function {ns}:campaign_complete"));
        }
        Verb::GiveItem {
            item,
            count,
            name,
            enchantments,
            ..
        } => {
            // One renderer for every stack a command writes: a given stack is
            // described exactly as a container fill's is, name and enchantments
            // alike, so a `give-item` and a `loot` entry for one item agree.
            let comp = container_stack_components(item, name.as_deref(), enchantments);
            // spec-0018: a quest beat arms the whole party (`@a`) unless the item
            // declares `carrier: "one"` — one quest prop, handed to the player
            // whose action earned it (`@s`), for the party to pass around. A
            // `carrier: "one"` in a scheduler-only bundle has no acting player and
            // is rejected at validate time (`DW0357`), so `has_actor` can only be
            // false here for the party-wide default.
            let one;
            let target = if eff.gives_to_one() && aud.has_actor() {
                // The one player, narrowed by the same box when the envelope
                // draws one.
                one = match effect_selector(plan, "@s", eff.within.as_ref()) {
                    Some(sel) => sel,
                    None => return,
                };
                one.as_str()
            } else {
                who
            };
            body.push(format!("give {target} {item}{comp} {count}"));
        }
        Verb::SetFlag { flag, .. } => {
            // Party state (spec-0018): one holder, so any player's action sets the
            // story flag for everyone — and a scheduled bundle can set it too (the
            // AUDIT-P0 `@s`-in-a-schedule class of bug is structurally gone).
            body.push(format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                plan::flag_score(flag.as_str())
            ));
        }
        // --- DSL v0.10 runtime state (spec-0031) ------------------------------
        // Each of the three is a plain `scoreboard players …` against the datum's
        // declared holder. `clear-state` WRITES the declared `initial` rather
        // than `reset`ting the score: a reset score is *absent*, and an absent
        // score makes `unless … matches` true — so a cleared datum would silently
        // satisfy a `not-equals` comparison against its own initial value.
        Verb::SetState { state, value, .. } => {
            body.push(format!(
                "scoreboard players set {} {} {value}",
                state_holder(plan, state),
                plan::state_score(state.as_str())
            ));
        }
        Verb::AddState { state, amount, .. } => {
            // `add` / `remove` rather than one signed `add`: vanilla's `add` takes
            // an unsigned operand and `remove` is its documented dual.
            let holder = state_holder(plan, state);
            let obj = plan::state_score(state.as_str());
            if *amount < 0 {
                body.push(format!(
                    "scoreboard players remove {holder} {obj} {}",
                    amount.unsigned_abs()
                ));
            } else {
                body.push(format!("scoreboard players add {holder} {obj} {amount}"));
            }
        }
        Verb::ClearState { state, .. } => {
            body.push(format!(
                "scoreboard players set {} {} {}",
                state_holder(plan, state),
                plan::state_score(state.as_str()),
                state_initial(plan, state)
            ));
        }
        // spec-0032. The verb is one `function` call, because everything a stake
        // drop does — the retention policy, the forfeit arithmetic, the
        // compile-time placement table, the marker — is shared between every site
        // that can leave one, and a bundle inlined at each site would be that
        // chain copied per firing.
        Verb::DropStake { stake, .. } => {
            body.push(format!(
                "function {ns}:stk_drop_{}",
                plan::safe_local(stake.as_str())
            ));
        }
        Verb::SpawnWave { wave, .. } => {
            body.push(format!(
                "function {ns}:spawn_{}",
                plan::safe_local(wave.as_str())
            ));
        }
        // --- DSL v0.4 effects ---
        Verb::Narrate {
            text, style, sound, ..
        } => {
            emit_narrate(text, *style, sound.as_deref(), who, body);
        }
        Verb::SetBlock { anchor, block, .. } => {
            if let Some(pos) = anchor_point_any(plan, anchor.as_str()) {
                body.push(format!("setblock {} {} {} {block}", pos[0], pos[1], pos[2]));
            }
        }
        // --- DSL v0.10 region writes (spec-0031) ---
        // The general spelling of what `open-gate`/`close-gate` do to a gate
        // anchor's box, through the same one command builder. An unresolvable box
        // emits nothing — a dangling `region/anchor` is `DW0142`/`DW0360` at
        // validation, not a silently mis-aimed fill here.
        Verb::FillRegion { .. } | Verb::ClearRegion { .. } => {
            if let Some((zone, block)) = eff.region_write()
                && let Some(region) = plan.zone_box(zone)
            {
                body.push(fill_region_command(region, block.unwrap_or(AIR), None));
            }
        }
        // spec-0080 §3.3: a repaint is `fillbiome` over the volume — the
        // region through `Plan::zone_box`, or the place's paint — with the
        // atmosphere's biome, or the ground biome for `atmosphere: null`,
        // through the one writer the bootstrap paint uses. An unresolvable
        // volume emits nothing: a dangling anchor or place is refused at
        // validation, never silently mis-aimed here.
        Verb::SetAtmosphere { atmosphere, .. } => {
            if let Some((min, max)) = crate::compiler::horizon::repaint_volume(plan, eff) {
                let biome = crate::compiler::horizon::biome_map(plan)
                    .biome_of(atmosphere.as_ref().map(|a| a.as_str()));
                body.extend(crate::compiler::atmosphere::fillbiome_lines(
                    min, max, &biome,
                ));
            }
        }
        // The same region write, over a box the PIECE declares (spec-0042 §2.4).
        // One `fill` per box of the way, in the metadata's own order, with the
        // block the metadata carries for a `laid` way and air for a `cleared`
        // one. Nothing here consults the effect for geometry, a block or a
        // direction, because the effect carries none of the three: an
        // unresolvable reference is `DW0547` long before emission, so a way that
        // reaches here has exactly one staged answer.
        Verb::OpenWay { .. } => {
            if let Some((piece, name)) = eff.way_write()
                && let Ok(way) = plan.ways.resolve(piece.as_str(), name)
            {
                let block = match way.sign {
                    crate::compiler::ways::Sign::Laid => way.block.as_str(),
                    crate::compiler::ways::Sign::Cleared => AIR,
                };
                for region in &way.boxes {
                    body.push(fill_region_command(*region, block, None));
                }
            }
        }
        Verb::DespawnNpc { npc, .. } => {
            body.push(format!("function {ns}:{}", despawn_npc_fn(npc.as_str())));
        }
        Verb::MoveNpc { npc, to, .. } => {
            body.push(format!(
                "function {ns}:{}",
                movenpc_fn(npc.as_str(), to, &crate::compiler::nav::gate_key(eff),)
            ));
        }
        Verb::Cutscene { .. } => {
            // Shape is policed at validation (`DW0199`); an unshaped cutscene
            // resolves to no shots and emits no call rather than a dangling one.
            if let Some(shots) = eff.cutscene_shots().filter(|s| !s.is_empty()) {
                let party = eff.cutscene_party().unwrap_or_default();
                body.push(format!("function {ns}:{}", cutscene_fn(&shots, party)));
            }
        }
        // --- DSL v0.5 effects (spec-0010) ---
        // Dimension-global instant cuts. The daylight/weather cycles are frozen by
        // environment sealing (`advance_time`/`advance_weather false`), so the set
        // state persists until the next cut. No selector: `/time set` and
        // `/weather` act on the whole dimension.
        Verb::SetTime { time, .. } => {
            // One token for every cut (spec-0081 §4.4): a keyword on day 0 is
            // its keyword; any other clock — a celestial statement, or a keyword
            // cut in a world whose moon stands on another day — is the integer.
            let world = plan.campaign.world.content.time;
            body.push(format!(
                "time set {}",
                time.token(time.clock(delvewright_dsl::TimeSite::Cut, world))
            ));
        }
        Verb::SetWeather { weather, .. } => {
            body.push(format!("weather {}", weather.token()));
        }
        // --- DSL v0.6 effects (spec-0012 checkpoints, spec-0014 stealth + sound) ---
        Verb::PlaySound {
            sound,
            at,
            volume,
            pitch,
            ..
        } => {
            emit_play_sound(plan, sound, at.as_ref(), *volume, *pitch, who, body);
        }
        Verb::DamagePlayers {
            amount,
            damage_type,
            ..
        } => {
            emit_damage_players(
                plan,
                *amount,
                eff.within.as_ref(),
                *damage_type,
                aud.selector(),
                body,
            );
        }
        // --- DSL v0.29 (spec-0068): a firework is an effect ---
        Verb::Firework {
            at,
            flight,
            explosions,
        } => {
            emit_firework(plan, at, *flight, explosions, body);
        }
        // --- spec-0092: a lightning bolt strikes at a mark ---
        Verb::Lightning { at } => {
            emit_lightning(plan, at, body);
        }
        // --- spec-0085: a particle is an effect ---
        Verb::Particle {
            particle,
            at,
            count,
            spread,
            speed,
        } => {
            emit_particle(plan, particle, at, *count, *spread, *speed, who, body);
        }
        Verb::SetCheckpoint { anchor, on_respawn } => {
            emit_set_checkpoint(plan, anchor.as_str(), on_respawn, body);
        }
        Verb::Bonfire {
            anchor, on_rest, ..
        } => {
            // Arm the rest affordance (spec-0016 §1): summon the interaction
            // entity the party right-clicks to rest. Guarded on absence so a
            // re-fired beat never stacks a second affordance (and so a `bonfire`
            // reached twice is idempotent). Nothing else happens here — the
            // checkpoint moves when the party REST, not when the beat fires.
            if let Some(bf) = plan.bonfire_for(anchor.as_str(), on_rest) {
                let v = ent_xyz(bf.pos);
                let i = bf.index;
                body.push(format!(
                    "execute unless entity @e[tag=dw_bonfire_{i}] run summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"dw_bonfire_{i}\"]}}",
                    v[0], v[1], v[2]
                ));
                // …and the visible hardware, under the same absence guard so a
                // re-fired beat never stacks a second one. A rest point the
                // player cannot see is the same soft-lock class as an invisible
                // unlock lever (`DW0420`). Never retired: a bonfire is not
                // consumed by resting at it.
                let hw = crate::compiler::affordance::hardware_tag(&format!("dw_bonfire_{i}"));
                body.push(format!(
                    "execute unless entity @e[tag={hw}] run {}",
                    affordance_hardware(v, &format!("dw_bonfire_{i}"), "minecraft:campfire")
                ));
            }
        }
        Verb::BeginStealth {
            zones, grace_ticks, ..
        } => {
            if let Some(beat) = plan.stealth_for(zones, *grace_ticks) {
                body.push(format!("function {ns}:stealth_begin_{}", beat.index));
            }
        }
        Verb::EndStealth => {
            body.push("scoreboard players set #stealth dw.sys 0".to_string());
        }
        // spec-0022 trap-payload verbs. Both lower to a call into a generated
        // function whose body is the PROVEN geometry (per-cell velocity vectors
        // / settled debris), so the effect site itself carries no coordinates.
        Verb::Volley { .. } => {
            body.push(format!("function {ns}:{}", volley_fn(eff)));
        }
        Verb::Collapse { .. } => {
            body.push(format!("function {ns}:{}", collapse_fn(eff)));
        }
        // --- DSL v0.6 actor staging effects (spec-0014) ---
        Verb::SpawnActor { actor, .. } => {
            body.push(format!(
                "function {ns}:spawn_actor_{}",
                plan::safe_local(actor.as_str())
            ));
        }
        Verb::DespawnActor { actor, style, .. } => {
            let declares_drops = plan
                .campaign
                .quests
                .content
                .actors
                .iter()
                .any(|a| a.id.as_str() == actor.as_str() && actor_declares_drops(a));
            emit_despawn_actor(ns, actor.as_str(), *style, declares_drops, body);
        }
        Verb::MoveActor { actor, to, .. } => {
            body.push(format!(
                "function {ns}:{}",
                moveactor_fn(actor.as_str(), to, &crate::compiler::nav::gate_key(eff),)
            ));
        }
        Verb::UnleashActor { actor, .. } => {
            body.push(format!(
                "function {ns}:unleash_{}",
                plan::safe_local(actor.as_str())
            ));
        }
        Verb::Sequence { .. } => {
            // The timeline keeps its actor (spec-0085 §3.2): keyed by the
            // audience it is started under, so the tag form (whose start
            // function tags `@s`) and the party form never share a body.
            body.push(format!("function {ns}:{}", sequence_fn(plan, eff, aud)));
        }
        Verb::SpawnNpc { npc, .. } => {
            body.push(format!("function {ns}:{}", spawn_npc_fn(npc.as_str())));
        }
        // --- spec-0082 assembly verbs: a call into the assembly's own
        // functions (`compiler::assembly`). ---
        Verb::SpawnAssembly { .. }
        | Verb::DespawnAssembly { .. }
        | Verb::PlayClip { .. }
        | Verb::ArmStrikes { .. } => {
            body.extend(crate::compiler::assembly::verb_lines(plan, &eff.verb).unwrap_or_default());
        }
        // --- DSL v0.10 status effects (spec-0031) -----------------------------
        // Vanilla `effect give` / `effect clear`, through the SAME formatter the
        // engine's own night-vision clock has used since v0.6
        // (`effect_give_command`) — the hard-coded case is now one configured use
        // of the general verb's emission rather than a private copy of it.
        //
        // No `tag=!dw_cutscene` guard, deliberately: a status effect is not
        // inherently harm (regeneration, night vision, glowing), and the engine's
        // pre-existing region-scoped grant has never carried one. Where an author
        // wants a beat to spare an observer, the `in` filter and the effect gate
        // both say so explicitly.
        Verb::GiveEffect { .. } => {
            // `who` already carries the envelope's `in` box.
            if let Some((effect, seconds, amplifier, hide, _)) = eff.give_effect() {
                body.push(effect_give_command(who, effect, seconds, amplifier, hide));
            }
        }
        Verb::ClearEffect { .. } => {
            if let Some((effect, _)) = eff.clear_effect() {
                let sel = who;
                // Vanilla's own two spellings: with an id, or bare for "all".
                body.push(match effect {
                    Some(id) => format!("effect clear {sel} {id}"),
                    None => format!("effect clear {sel}"),
                });
            }
        }
        // --- DSL v0.10 teleport (spec-0031) -----------------------------------
        // ONE command, and its selector is the volume — never the effect's
        // audience. `who` is deliberately unused here: a teleport moves what is
        // INSIDE the box, and a box does not have a party. The selector carries
        // the six box terms plus the one class exclusion every box-narrowed
        // entity selector in this engine carries — `tag=!dw_fixture`, and no
        // `type=`, no `limit=`, no `sort=`. That is what makes the selection
        // total over BODIES, and `crates/delvec/tests/v10_teleport.rs` asserts
        // exactly that against the emitted string. See `Verb::Teleport`
        // for why a machinery-TYPE exemption (which `lethal_volumes[]` must
        // carry) would be wrong here, and `crate::compiler::affordance` for the class that
        // stands in its place.
        Verb::Teleport { .. } => {
            // A call into the generated function, exactly as `volley` and
            // `collapse` do: the body is proven geometry, and a body that only
            // ever exists inline is a body no runtime test can call.
            if teleport_command(plan, eff).is_some() {
                body.push(format!("function {ns}:{}", teleport_fn(eff)));
            }
        }
    }
}

/// `sel` (a player selector ending in its argument list's `]`) with the
/// observation tag excluded: the one spelling for a box that means the bodies
/// in it.
pub(super) fn unwatched(sel: String) -> String {
    match sel.strip_suffix(']') {
        Some(head) => format!("{head},tag=!{CUTSCENE_TAG}]"),
        None => format!("{sel}[tag=!{CUTSCENE_TAG}]"),
    }
}

/// The `effect give`/`effect clear` target selector for a v0.10 status-effect
/// verb: the effect's audience, narrowed by the declared `in` box when there is
/// one.
///
/// `None` when the filter's anchor does not resolve — referential validation
/// already reports that (`DW0142`), and emitting a selector with a blank box
/// would be an invalid command rather than a diagnosis.
pub(super) fn effect_selector(
    plan: &Plan,
    who: &str,
    within: Option<&delvewright_dsl::StealthZone>,
) -> Option<String> {
    match within {
        None => Some(who.to_string()),
        Some(zone) => {
            let (lo, hi) = plan.zone_box(zone)?;
            Some(format!("{who}[{}]", box_selector_args(lo, hi)))
        }
    }
}

/// Vanilla's `effect give`, always in its full five-token form.
///
/// The engine has emitted this since v0.6 and exposed no verb for it; this is the
/// one place that writes the command, used by both the author-facing
/// `give-effect` and the night-vision area mitigation
/// ([`night_vision_fns`]). The full form — duration, amplifier and
/// `hideParticles` all present — is what the mitigation already emitted, so
/// routing it through here is byte-identical for every existing campaign, and it
/// leaves nothing to a vanilla default that a future version could re-pick.
pub(super) fn effect_give_command(
    selector: &str,
    effect: &str,
    seconds: u32,
    amplifier: u32,
    hide_particles: bool,
) -> String {
    format!("effect give {selector} {effect} {seconds} {amplifier} {hide_particles}")
}

/// Emit a `play-sound` effect (DSL v0.6). `who` is the audience selector
/// (spec-0018: `@a` for a party beat, `@s` inside a solo `on_respawn`/`on_caught`
/// bundle). An `at: anchor` sound carries absolute coordinates, so every listener
/// hears it in the same place; the default `players` target is
/// listener-relative and, when a volume/pitch is declared (which forces an
/// explicit position), is emitted through `execute as <who> at @s run … ~ ~ ~` so
/// `~ ~ ~` resolves at each listener rather than at the command's own position.
/// `at: actor` never reaches emission — no position resolves for a live actor,
/// so it is rejected at validate-time (`DW0335`).
pub(super) fn emit_play_sound(
    plan: &Plan,
    sound: &str,
    at: Option<&delvewright_dsl::SoundAt>,
    volume: Option<f64>,
    pitch: Option<f64>,
    who: &str,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::SoundAt;
    // Canonicalize a bare id to the default namespace so the emitted command is
    // explicit (`playsound` accepts either form).
    let sound = if sound.contains(':') {
        sound.to_string()
    } else {
        format!("minecraft:{sound}")
    };
    // spec-0085 §4.4: a sound in the listener's own frame. The listener inside the
    // loop is `@s`, never the audience selector — `<who>` there would play each
    // sound once per listener at every listener's behind.
    if let Some(SoundAt::Players { offset }) = at
        && *offset != [0, 0, 0]
    {
        let mut cmd = format!(
            "execute as {who} at @s rotated ~ 0 positioned ^{} ^{} ^{} run playsound {sound} master @s ~ ~ ~",
            offset[0], offset[1], offset[2]
        );
        if volume.is_some() || pitch.is_some() {
            cmd.push_str(&format!(" {}", volume.unwrap_or(1.0)));
            if let Some(pt) = pitch {
                cmd.push_str(&format!(" {pt}"));
            }
        }
        body.push(cmd);
        return;
    }
    let pos = match at {
        Some(SoundAt::Anchor { anchor, offset }) => match anchor_point_any(plan, anchor.as_str()) {
            Some(p) => {
                let p = delvewright_dsl::offset_cell(p, *offset);
                Some(format!("{} {} {}", p[0], p[1], p[2]))
            }
            None => return, // unresolved anchor (referential validation reports it)
        },
        Some(SoundAt::Actor { .. }) => return, // unsupported: DW0335 at validate-time
        _ => None,                             // `players` (default): player-relative
    };
    let listener_relative = pos.is_none() && (volume.is_some() || pitch.is_some());
    let mut cmd = format!(
        "playsound {sound} master {}",
        if listener_relative { "@s" } else { who }
    );
    if pos.is_some() || volume.is_some() || pitch.is_some() {
        let p = pos.unwrap_or_else(|| "~ ~ ~".to_string());
        cmd.push_str(&format!(" {p}"));
        if volume.is_some() || pitch.is_some() {
            cmd.push_str(&format!(" {}", volume.unwrap_or(1.0)));
            if let Some(pt) = pitch {
                cmd.push_str(&format!(" {pt}"));
            }
        }
    }
    if listener_relative && who != "@s" {
        // `~ ~ ~` is the COMMAND's position, not the listener's — rebind so each
        // party member hears it at their own feet.
        cmd = format!("execute as {who} at @s run {cmd}");
    }
    body.push(cmd);
}

/// Emit a `particle` effect (spec-0085 §4.3): one vanilla `particle` command,
/// always in `force` mode, its viewers the effect's audience.
///
/// At `players` the particle is spawned at each addressed player — `execute as
/// <who> at @s run particle <id> ~ ~ ~ … force @s`, the viewer being the player
/// it is spawned at, so a full-screen `elder_guardian` is drawn for that player
/// alone. At a mark it is spawned at the cell's horizontal centre on the mark's
/// plane and shown to `<who>`.
///
/// `force` is written, never chosen: in `normal` mode the game sends a particle
/// 32 blocks and the client may drop it at the Minimal particle setting; in
/// `force` mode it is sent 512 blocks and drawn at every setting [cited —
/// *Commands/particle*]. An authored beat is meant to be seen.
///
/// An unresolved mark emits nothing and is `DW0360` long before here.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_particle(
    plan: &Plan,
    particle: &str,
    at: &delvewright_dsl::ParticleAt,
    count: Option<u32>,
    spread: Option<[f64; 3]>,
    speed: Option<f64>,
    who: &str,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::ParticleAt;
    let id = if particle.contains(':') {
        particle.to_string()
    } else {
        format!("minecraft:{particle}")
    };
    let [dx, dy, dz] = spread.unwrap_or([0.0, 0.0, 0.0]);
    let tail = format!(
        "{dx} {dy} {dz} {} {} force",
        speed.unwrap_or(0.0),
        count.unwrap_or(1)
    );
    match at {
        ParticleAt::Players(_) => {
            body.push(format!(
                "execute as {who} at @s run particle {id} ~ ~ ~ {tail} @s"
            ));
        }
        ParticleAt::Mark(mark) => {
            let Some(anchor) = anchor_point_any(plan, mark.anchor.as_str()) else {
                return; // unresolved anchor (`DW0360` owns it)
            };
            let c = mark.cell(anchor);
            let x = f64::from(c[0]) + 0.5;
            let z = f64::from(c[2]) + 0.5;
            body.push(format!("particle {id} {x} {} {z} {tail} {who}", c[1]));
        }
    }
}

/// Emit a `damage-players` effect (DSL v0.6). `who` is the audience selector
/// (spec-0018): `@a` on a party beat — the hazard is a fact about the delve, so
/// it hits every party member once — and `@s` inside a solo `on_caught` /
/// `on_respawn` bundle, where exactly the one player it belongs to is hurt.
/// `amount` is in half-hearts (1 HP each); the type is a curated vanilla damage
/// type (default `minecraft:generic`). A `within` box narrows to players standing
/// inside the anchor-centred AABB — the same box model the stealth zone check
/// uses (no double-hit: each player is judged on their own position).
///
/// **`/damage` takes ONE entity** (the vendored command tree says
/// `amount: "single"`, and 1.21.11 refuses to load a whole function containing
/// `damage @a[…] …`), so the party form is reached by re-binding —
/// `execute as @a[…] run damage @s …` — not by widening the target. The solo
/// form keeps its `if entity @s[…]` guard.
///
/// Every form is guarded by `tag=!dw_cutscene`: a player watching a cutscene is
/// never harmed by campaign machinery (see [`CUTSCENE_TAG`]).
pub(super) fn emit_damage_players(
    plan: &Plan,
    amount: u32,
    within: Option<&delvewright_dsl::StealthZone>,
    damage_type: Option<delvewright_dsl::DamageKind>,
    who: &str,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::DamageKind;
    let kind = damage_type.unwrap_or(DamageKind::Generic).id();
    let filters = match within {
        Some(zone) => {
            // A blank box when the anchor is unresolved (referential validation
            // reports that, DW0142) — emit nothing rather than an invalid selector.
            let Some(pos) = anchor_point_any(plan, zone.anchor.as_str()) else {
                return;
            };
            let lo = [
                pos[0] - zone.extent[0] as i32,
                pos[1] - zone.extent[1] as i32,
                pos[2] - zone.extent[2] as i32,
            ];
            let size = [
                2 * zone.extent[0] as i32,
                2 * zone.extent[1] as i32,
                2 * zone.extent[2] as i32,
            ];
            format!(
                "x={},dx={},y={},dy={},z={},dz={},tag=!{CUTSCENE_TAG}",
                lo[0], size[0], lo[1], size[1], lo[2], size[2]
            )
        }
        // The bare form still needs the cutscene guard (see CUTSCENE_TAG: a
        // cutscene is pure observation — campaign machinery never harms a player
        // who is only watching).
        None => format!("tag=!{CUTSCENE_TAG}"),
    };
    if who == "@s" {
        body.push(format!(
            "execute if entity @s[{filters}] run damage @s {amount} {kind}"
        ));
    } else {
        body.push(format!(
            "execute as {who}[{filters}] run damage @s {amount} {kind}"
        ));
    }
}

/// Emit a `narrate` line in its channel (DSL v0.4). `chat` = `tellraw`; `title`
/// / `subtitle` = the vanilla `title` command (a subtitle is paired with a blank
/// title so it renders on its own). An optional sound plays alongside. `who` is
/// the audience selector (spec-0018): the story is told to the whole party
/// (`@a`), except inside a solo `on_respawn`/`on_caught` bundle (`@s`).
pub(super) fn emit_narrate(
    text: &str,
    style: Option<delvewright_dsl::NarrateStyle>,
    sound: Option<&str>,
    who: &str,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::NarrateStyle;
    let comp = tr(text);
    match style.unwrap_or(NarrateStyle::Chat) {
        NarrateStyle::Chat => body.push(format!("tellraw {who} {comp}")),
        NarrateStyle::Title => body.push(format!("title {who} title {comp}")),
        NarrateStyle::Subtitle => {
            body.push(format!("title {who} title {}", json!({ "text": " " })));
            body.push(format!("title {who} subtitle {comp}"));
        }
        // Large-glyph "art" title through the delve's custom resource-pack font
        // (`delve:art`, DSL v0.6). The font is uppercase-only (glyph coverage is
        // checked at compile time, DW0328), so render uppercase.
        NarrateStyle::Art => {
            let art = tr_with(text, &[("font", json!("delve:art"))]);
            body.push(format!("title {who} title {art}"));
        }
        // DSL v0.11: the reply strip above the hotbar. This is the command every
        // compiler-written reply has always used — a sealed gate's answer, a
        // checkpoint return, the lobby count — reached at last by the general
        // verb, so a campaign can write its own replies instead of the engine
        // owning them one verb at a time.
        NarrateStyle::Actionbar => body.push(format!("title {who} actionbar {comp}")),
    }
    if let Some(s) = sound {
        body.push(format!("playsound {s} player {who}"));
    }
}

/// Every quest effect in the campaign, flattened through `sequence` steps and
/// `move-actor` `on_arrive` (spec-0014) so nested lifecycle/cutscene/actor targets
/// are collected. Pre-0.6 campaigns have no nesting, so this equals the shallow
/// list (byte-identical).
///
/// The roots come from [`crate::compiler::plan::for_each_effect_root`] — the one enumeration
/// the gate scans and the staged-walk timeline also walk. What the emitter
/// generates functions for and what the proofs check are therefore the same set by
/// construction: a `sequence`/`cutscene`/`move-actor` in **any** root gets its
/// generated function, and none of the four walks can quietly grow a different
/// idea of where effects live.
pub(super) fn all_campaign_effects(c: &delvewright_dsl::Campaign) -> Vec<&QuestEffect> {
    let mut out = Vec::new();
    crate::compiler::plan::for_each_effect_root(c, &mut |_site, effs| {
        for e in effs {
            push_effect_deep(e, &mut out);
        }
    });
    out
}

/// Push `e` and every transitively nested effect, descending through every nested
/// effect list ([`QuestEffect::nested_effect_lists`]: `sequence` steps,
/// `set-checkpoint` `on_respawn`, `begin-stealth` `on_caught`, `move-actor`
/// `on_arrive`). Completeness matters: e.g. a `sequence` nested in an `on_respawn`
/// must be reached here so `sequence_fns` generates its `seq_…` function — the
/// `emit_quest_effect` for the nested effect emits a `function` call to it.
pub(super) fn push_effect_deep<'a>(e: &'a QuestEffect, out: &mut Vec<&'a QuestEffect>) {
    out.push(e);
    for list in e.nested_effect_lists() {
        for inner in list {
            push_effect_deep(inner, out);
        }
    }
}

/// Every effect under `e` with the audience the emitter lowers it under —
/// [`push_effect_deep`] carrying the command source down each nesting site by
/// the DSL's own [`delvewright_dsl::NestedDispatch`].
///
/// One nesting site is lowered twice, and both readings are pushed: a
/// `bonfire`'s `on_rest` runs under [`Audience::Scheduled`] in
/// `bonfire_rest_<i>` and under [`Audience::Solo`] in the respawn path's
/// `cp_on_respawn_<i>` (see `emit_bonfire_functions`), so a timeline inside it
/// is called from both and owes a body for both.
pub(super) fn push_effect_deep_audience<'a>(
    e: &'a QuestEffect,
    aud: Audience,
    out: &mut Vec<(&'a QuestEffect, Audience)>,
) {
    use delvewright_dsl::NestedDispatch;
    out.push((e, aud));
    for (list, how) in e.nested_effect_dispatch() {
        let inner: &[Audience] = match how {
            NestedDispatch::Inherit => &[aud],
            NestedDispatch::Player => &[Audience::Solo],
            NestedDispatch::Server if matches!(e.verb, Verb::Bonfire { .. }) => {
                &[Audience::Scheduled, Audience::Solo]
            }
            NestedDispatch::Server => &[Audience::Scheduled],
        };
        for &a in inner {
            for x in list {
                push_effect_deep_audience(x, a, out);
            }
        }
    }
}
