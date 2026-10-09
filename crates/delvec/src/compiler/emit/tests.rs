use super::*;

/// **The emitter's audience per effect root equals the DSL's own answer**,
/// over the closed root set, in both directions.
///
/// `EffectRootKind::runs_with_acting_player` is what `DW0503` trusts when it
/// decides whether a `player`-scoped runtime datum (spec-0031) may be read or
/// written inside a root's bundle. If a root's emitted audience ever moved
/// without that answer moving with it, a validated campaign would emit `@s`
/// into a function with no command source — a silent runtime failure with
/// every check green. This is the bind. An eighth root fails it until both
/// sides name it.
#[test]
fn root_audience_matches_the_dsl() {
    for kind in delvewright_dsl::EffectRootKind::ALL {
        assert_eq!(
            root_audience(kind).has_actor(),
            kind.runs_with_acting_player(),
            "root `{}`: the emitter and `EffectRootKind::runs_with_acting_player` disagree \
             about whether its bundle has an acting player",
            kind.label()
        );
    }
    // Binding: the loop must have examined every root, and both answers must
    // actually occur — an assertion that only ever saw `true` would pass on a
    // constant.
    let all = delvewright_dsl::EffectRootKind::ALL;
    assert_eq!(all.len(), delvewright_dsl::EffectRootKind::COUNT);
    assert!(all.iter().any(|k| k.runs_with_acting_player()));
    assert!(all.iter().any(|k| !k.runs_with_acting_player()));
}

#[test]
fn facing_yaw_matches_mc_convention() {
    // MC yaw: south=0, west=90, north=180, east=270.
    assert_eq!(facing_yaw(Some("south")), 0);
    assert_eq!(facing_yaw(Some("west")), 90);
    assert_eq!(facing_yaw(Some("north")), 180);
    assert_eq!(facing_yaw(Some("east")), 270);
    assert_eq!(facing_yaw(None), 0);
}

#[test]
fn snbt_string_is_a_plain_quoted_component() {
    // A bare quoted SNBT string is a valid text component (renders literally),
    // unlike the old `'{"text":…}'` JSON-string form.
    assert_eq!(
        snbt_string("Hedric of the Watch"),
        "\"Hedric of the Watch\""
    );
    // Backslash and double-quote are escaped.
    assert_eq!(snbt_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
}

#[test]
fn marker_name_fields_never_leak_a_raw_id() {
    // A titled marker carries its title (byte-identical to the old behavior).
    assert_eq!(
        marker_name_fields(Some("Unbar the Inner Door")),
        "CustomName:\"Unbar the Inner Door\",CustomNameVisible:1b,"
    );
    // An untitled objective yields NO name fields — the marker still glows but
    // never surfaces its raw objective id (e.g. `obj/door`) to players.
    assert_eq!(marker_name_fields(None), "");
}

#[test]
fn default_equipment_arms_only_naturally_armed_mobs() {
    // wither_skeleton → stone sword via the component-era `equipment` NBT
    // with a zero `drop_chances` (1.21.11 ignores legacy `HandItems`).
    let ws = default_equipment("minecraft:wither_skeleton").unwrap();
    assert!(ws.contains("equipment:{mainhand:{id:\"minecraft:stone_sword\",count:1}}"));
    assert!(ws.contains("drop_chances:{mainhand:0.0f}"));
    // No trace of the legacy, silently-ignored form.
    assert!(!ws.contains("HandItems"));
    assert!(!ws.contains("HandDropChances"));
    // skeleton/stray → bow.
    assert!(
        default_equipment("skeleton")
            .unwrap()
            .contains("minecraft:bow")
    );
    assert!(
        default_equipment("minecraft:stray")
            .unwrap()
            .contains("minecraft:bow")
    );
    // zombie stays unarmed; drowned's trident is not a default.
    assert!(default_equipment("minecraft:zombie").is_none());
    assert!(default_equipment("minecraft:drowned").is_none());
}

// --- DSL v0.6 actor emission (spec-0014) ---

fn mk_actor(id: &str, entity: &str, vulnerable: bool) -> delvewright_dsl::Actor {
    delvewright_dsl::Actor {
        on_kill: None,
        id: delvewright_dsl::ActorId(id.to_string()),
        entity: entity.to_string(),
        name: Some("Boss".to_string()),
        skin: None,
        anchor: delvewright_dsl::AnchorId("anchor/stage".to_string()),
        offset: [0, 0, 0],
        facing: Some(delvewright_dsl::Facing::West),
        vulnerable,
        equipment: None,
        drops: Vec::new(),
        attributes: None,
        tier: None,
        traversal: None,
        health_bar: None,
    }
}

#[test]
fn puppet_summon_is_noai_no_loot_and_tagged() {
    let a = mk_actor("actor/giant", "minecraft:warden", false);
    let s = actor_puppet_summon("dw", &a, [10, 65, 20], facing_yaw(Some("west")));
    assert!(
        s.starts_with("summon minecraft:warden 10.5 65.0 20.5 "),
        "puppet stands at the CENTRE of its cell, not the four-column corner: {s}"
    );
    assert!(s.contains("NoAI:1b") && s.contains("Silent:1b") && s.contains("NoGravity:1b"));
    assert!(s.contains("Invulnerable:1b"));
    assert!(s.contains("DeathLootTable:\"minecraft:empty\""));
    assert!(s.contains("dw_actor_giant") && s.contains("dw_pup_giant"));
    assert!(s.contains("Rotation:[90f,0f]"));
    assert!(
        !s.contains("knockback_resistance"),
        "invulnerable puppet has no kb attr"
    );
}

#[test]
fn vulnerable_puppet_is_damageable_but_knockback_immune() {
    let a = mk_actor("actor/creep", "minecraft:zombie", true);
    let s = actor_puppet_summon("dw", &a, [0, 64, 0], 0);
    assert!(
        s.contains("Invulnerable:0b"),
        "vulnerable puppet takes damage"
    );
    assert!(
        s.contains("knockback_resistance") && s.contains("base:1.0"),
        "vulnerable puppet stays knockback-immune: {s}"
    );
}

#[test]
fn skinned_puppet_is_a_mannequin() {
    let mut a = mk_actor("actor/keeper", "minecraft:warden", false);
    a.skin = Some(delvewright_dsl::NpcSkin {
        texture_id: "giant-idle".to_string(),
        model: delvewright_dsl::SkinModel::Wide,
        hidden_layers: vec![],
    });
    let s = actor_puppet_summon("dw", &a, [1, 2, 3], 180);
    assert!(
        s.starts_with("summon minecraft:mannequin 1.5 2.0 3.5 "),
        "mannequin stands at the centre of its cell: {s}"
    );
    assert!(s.contains("profile:{texture:\"delvewright:npc/giant-idle\",model:\"wide\"}"));
    assert!(s.contains("dw_pup_keeper"));
}

/// spec-0097 §5: a skin's hidden layers ride both mannequin summons, in
/// authored order, and an empty list writes nothing at all.
#[test]
fn a_mannequin_hides_the_layers_its_skin_names() {
    use delvewright_dsl::SkinLayer;
    let mut a = mk_actor("actor/keeper", "minecraft:warden", false);
    a.skin = Some(delvewright_dsl::NpcSkin {
        texture_id: "giant-idle".to_string(),
        model: delvewright_dsl::SkinModel::Wide,
        hidden_layers: vec![SkinLayer::Hat, SkinLayer::Jacket],
    });
    let s = actor_puppet_summon("dw", &a, [1, 2, 3], 180);
    assert!(
        s.contains("model:\"wide\"},hidden_layers:[\"hat\",\"jacket\"],immovable:1b"),
        "{s}"
    );
    a.skin.as_mut().unwrap().hidden_layers.clear();
    let s = actor_puppet_summon("dw", &a, [1, 2, 3], 180);
    assert!(!s.contains("hidden_layers"), "{s}");
}

/// spec-0096 × spec-0097: a skinned actor's styled name rides its mannequin
/// `description` as a styled SNBT component — never as raw brackets — beside the
/// layers its skin hides.
#[test]
fn a_skinned_actor_wears_a_styled_name_and_its_hidden_layers() {
    use delvewright_dsl::SkinLayer;
    let mut a = mk_actor("actor/keeper", "minecraft:warden", false);
    a.name = Some("The [[bold|Keeper]]".to_string());
    a.skin = Some(delvewright_dsl::NpcSkin {
        texture_id: "giant-idle".to_string(),
        model: delvewright_dsl::SkinModel::Wide,
        hidden_layers: vec![SkinLayer::Hat],
    });
    let s = actor_puppet_summon("dw", &a, [1, 2, 3], 180);
    assert!(s.contains("hidden_layers:[\"hat\"]"), "{s}");
    assert!(
        s.contains("description:{extra:[{text:\"The \"},{bold:true,text:\"Keeper\"}],text:\"\"}"),
        "{s}"
    );
    assert!(!s.contains("[["), "no markup reaches the summon: {s}");
}

/// A `skin` is a costume, not a lobotomy: a skinned actor is the same body
/// with a different dress, so everything the author declared about the body
/// rides it. Before this, the mannequin branch carried none of `vulnerable`,
/// `attributes` or `equipment` — an actor that ships armed and tunable
/// shipped naked and vanilla the moment it was given a face.
#[test]
fn a_skinned_puppet_keeps_everything_declared_about_its_body() {
    let mut a = mk_actor("actor/keeper", "minecraft:zombie", true);
    a.skin = Some(delvewright_dsl::NpcSkin {
        texture_id: "guard".to_string(),
        model: delvewright_dsl::SkinModel::Wide,
        hidden_layers: vec![],
    });
    a.equipment = Some(delvewright_dsl::MobEquipment {
        head: Some(EquipItem::Plain("minecraft:netherite_helmet".to_string())),
        chest: Some(EquipItem::Plain(
            "minecraft:netherite_chestplate".to_string(),
        )),
        legs: None,
        feet: None,
        main_hand: Some(EquipItem::Plain("minecraft:netherite_sword".to_string())),
        off_hand: None,
        body: None,
        saddle: None,
    });
    a.attributes = Some(delvewright_dsl::MobAttributes {
        max_health: Some(40.0),
        attack_damage: Some(9.0),
        movement_speed: None,
        follow_range: None,
    });
    let s = actor_puppet_summon("dw", &a, [1, 2, 3], 180);

    assert!(
        s.contains("Invulnerable:0b"),
        "a `vulnerable` skinned puppet takes damage like any other body: {s}"
    );
    assert!(
        s.contains("{id:\"minecraft:knockback_resistance\",base:1.0}"),
        "a vulnerable puppet stays knockback-immune whatever it is wearing: {s}"
    );
    assert!(
        s.contains("{id:\"minecraft:max_health\",base:40.0}")
            && s.contains("{id:\"minecraft:attack_damage\",base:9.0}"),
        "declared `attributes` must reach the mannequin — vanilla merges them \
         over its defaults (live-probed: `max_health` 40 ⇒ `Health: 40.0f`): {s}"
    );
    assert!(
        s.contains("mainhand:{id:\"minecraft:netherite_sword\"")
            && s.contains("head:{id:\"minecraft:netherite_helmet\"")
            && s.contains("chest:{id:\"minecraft:netherite_chestplate\",count:1}"),
        "declared `equipment` must reach the mannequin, which wears and renders \
         it: {s}"
    );

    // The loot half is a vanilla limit and is stated as one: a mannequin is a
    // `LivingEntity`, not a `Mob`, so neither `DeathLootTable` nor
    // `drop_chances` is part of its save data (both read back
    // `Found no elements matching` on the pinned server) and a killed one
    // drops nothing at all. Writing them here would be a claim the world does
    // not carry — see [`body_carries_loot_nbt`].
    assert!(
        !s.contains("DeathLootTable") && !s.contains("drop_chances"),
        "a mannequin body carries no Mob loot NBT: {s}"
    );
}

#[test]
fn twin_summon_has_ai_and_no_puppet_marker() {
    let a = mk_actor("actor/giant", "minecraft:warden", false);
    let s = actor_twin_summon("dw", &a, "~ ~ ~");
    assert!(s.starts_with("summon minecraft:warden ~ ~ ~ "));
    assert!(!s.contains("NoAI"), "the twin has real AI");
    assert!(s.contains("dw_actor_giant") && !s.contains("dw_pup"));
    assert!(s.contains("PersistenceRequired:1b"));
}

/// v0.9: a `despawn-actor` on a drop-declaring actor strips the
/// declaration off the body before killing it. `/kill` is an ordinary death
/// and a preserved slot survives a non-player kill, so without this a souls
/// re-seat would shower the party with the elite's own axe every rest.
#[test]
fn despawn_strips_declared_drops_first() {
    let mut cmds = Vec::new();
    emit_despawn_actor(
        "dw",
        "actor/giant",
        delvewright_dsl::DespawnStyle::Kill,
        true,
        &mut cmds,
    );
    assert_eq!(cmds.len(), 2, "strip then kill: {cmds:?}");
    assert!(
        cmds[0].starts_with("execute as @e[tag=dw_actor_giant] run data merge entity @s ")
            && cmds[0].contains("mainhand:0.0f")
            && cmds[0].contains("feet:0.0f")
            && cmds[0].contains("DeathLootTable:\"minecraft:empty\""),
        "{cmds:?}"
    );
    assert_eq!(cmds[1], "kill @e[tag=dw_actor_giant]");
}

#[test]
fn despawn_styles_differ() {
    let mut kill = Vec::new();
    emit_despawn_actor(
        "dw",
        "actor/giant",
        delvewright_dsl::DespawnStyle::Kill,
        false,
        &mut kill,
    );
    assert_eq!(kill, vec!["kill @e[tag=dw_actor_giant]".to_string()]);
    let mut vanish = Vec::new();
    emit_despawn_actor(
        "dw",
        "actor/giant",
        delvewright_dsl::DespawnStyle::Vanish,
        false,
        &mut vanish,
    );
    // `vanish` leaves unseen: the body is moved down its OWN column (the
    // `at @s` — a server-source `tp` resolves `~ ~` at world spawn), frozen
    // under the world with every tag replaced, and killed there by the sweep
    // a full delay later. Nothing in it kills the body where it stood.
    assert_eq!(
        vanish,
        vec![
            "execute if entity @e[tag=dw_actor_giant] run schedule function dw:unseen_sweep 5t replace".to_string(),
            "execute as @e[tag=dw_actor_giant] on passengers run ride @s dismount".to_string(),
            "execute as @e[tag=dw_actor_giant] at @s run tp @s ~ -128 ~".to_string(),
            "execute as @e[tag=dw_actor_giant] run data merge entity @s {Tags:[\"dw_unseen\"],NoGravity:1b,NoAI:1b,Silent:1b}".to_string(),
        ]
    );
    assert!(!vanish.iter().any(|l| l.starts_with("kill ")), "{vanish:?}");
}

/// **Despawn-if-exists**: every actor-lifecycle verb has to survive the body
/// simply not being there any more.
///
/// This is not hypothetical. An `unleash`ed warden is a *real* vanilla warden,
/// and vanilla wardens remove themselves — the ancient-city dig-down burrows
/// the mob out of the world on its own schedule. So by the time a later beat
/// fires `despawn-actor` (and hands off to the NPC), the entity the story
/// thinks it is dismissing may already be gone. The staging must be a no-op
/// then, never a hard error that takes the rest of the bundle's function down
/// with it — and there is no dangling tag to clean up, because tags live on
/// the entity and a removed entity takes its tags with it.
///
/// The property is structural: the body is always addressed through a **plain
/// multi-entity tag selector** — `@e[tag=dw_actor_<id>]` with no `limit=1` —
/// so a zero-match run affects nothing and the function continues. A
/// single-entity-arity form here would be exactly the 1.21.11 load-failure
/// class the command-tree check guards elsewhere.
///
/// A bare `@s` is equally forbidden (nothing binds it in a scheduled bundle),
/// but an `@s` **bound by an enclosing `execute as`** in the same command is
/// fine and is what `vanish` uses: `execute as @e[tag=…] at @s run tp @s …`
/// runs its body zero times when nothing matches, and the `at @s` is required
/// for the relative `~ -128 ~` to resolve at the body rather than at the
/// command source. So the check is "every `@s` is bound", not "no `@s`" —
/// the latter would reject a strictly more correct emission.
#[test]
fn actor_lifecycle_verbs_are_no_ops_when_the_body_is_already_gone() {
    for style in [
        delvewright_dsl::DespawnStyle::Kill,
        delvewright_dsl::DespawnStyle::Vanish,
    ] {
        let mut cmds = Vec::new();
        emit_despawn_actor("dw", "actor/giant", style, false, &mut cmds);
        assert!(!cmds.is_empty());
        for c in &cmds {
            assert!(
                c.contains("@e[tag=dw_actor_giant]"),
                "targets the body tag, so no match = no effect: {c}"
            );
            assert!(
                !c.contains("limit=1"),
                "no single-entity arity: a zero-match run must not fail the function: {c}"
            );
            if c.contains("@s") {
                assert!(
                    c.contains("execute as @e[tag=dw_actor_giant]"),
                    "every `@s` must be bound by an enclosing `execute as`: {c}"
                );
            }
        }
    }
    // The dual: re-staging after the body removed itself must work. The re-cage
    // summon is `execute unless entity` guarded, so it fires exactly when the
    // body is absent and no-ops when it is not.
    let a = mk_actor("actor/giant", "minecraft:warden", false);
    let spawn = format!(
        "execute unless entity @e[tag=dw_actor_giant] run {}",
        actor_puppet_summon("dw", &a, [0, 64, 0], 0)
    );
    assert!(
        spawn.starts_with("execute unless entity @e[tag=dw_actor_giant] run summon "),
        "re-cage is idempotent and works from nothing: {spawn}"
    );
}
