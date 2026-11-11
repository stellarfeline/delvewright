//! Loot containers.

use super::*;

/// `setup_finish` commands for container fills (spec-0021): give each declared
/// container its contents with `item replace block … container.<slot>`, the same
/// deterministic mechanism a trap dispenser and a `collect` chest already use —
/// no raw NBT, no loot tables, no RNG.
///
/// **Slot assignment is positional**: the nth declared stack lands in
/// `container.<n>`. That is the whole determinism story (ADR-0006) — the same
/// DSL always produces the same chest, byte for byte, with no shuffling and no
/// seeded placement to reproduce.
///
/// The container itself is never placed here; it is prefab furniture, and
/// `DW0431` has already proven one is really there.
pub(super) fn loot_setup(
    loot: &[crate::compiler::plan::LootPlan],
    holders: &[crate::compiler::holders::OwnedHolder],
) -> Vec<String> {
    let mut out = Vec::new();
    for l in loot {
        // A fill of a holder ENTITY is the holder setup's (spec-0111 §4.1),
        // and a fill that waits for its beat is its own function's (§6).
        if l.by_effect
            || holders
                .iter()
                .any(|o| o.loot.as_deref() == Some(l.id.as_str()))
        {
            continue;
        }
        out.extend(block_fill_lines(l));
    }
    out
}

/// The name of a `by-effect` fill's own function (spec-0134 §6):
/// `fill_<loot>`, called by `fill-loot`.
pub(crate) fn fill_fn(loot_id: &str) -> String {
    format!("fill_{}", crate::compiler::plan::safe_local(loot_id))
}

/// **Every `by-effect` fill's own function** (spec-0134 §6): the same lines
/// the fill would have run in `setup_finish` — a container's `item replace
/// block`, a holder entity's `item replace entity` — run when a `fill-loot`
/// fires. Empty for a campaign with none, so its function set is unchanged.
pub(super) fn loot_fill_fns(
    loot: &[crate::compiler::plan::LootPlan],
    holders: &[crate::compiler::holders::OwnedHolder],
) -> Vec<(String, String)> {
    loot.iter()
        .filter(|l| l.by_effect)
        .map(|l| {
            let body = match holders
                .iter()
                .find(|o| o.loot.as_deref() == Some(l.id.as_str()))
            {
                Some(o) => super::holders::holder_loot_lines(o),
                None => block_fill_lines(l),
            };
            (fill_fn(&l.id), lines(&body))
        })
        .collect()
}

/// One container fill's `item replace block` lines.
fn block_fill_lines(l: &crate::compiler::plan::LootPlan) -> Vec<String> {
    let mut out = Vec::new();
    {
        let c = l.cell;
        for (n, it) in l.items.iter().enumerate() {
            // A named slot is the stack's own (spec-0111 §3.3); an unnamed
            // stack lands positionally.
            let slot = it.slot.clone().unwrap_or_else(|| format!("container.{n}"));
            out.push(format!(
                "item replace block {} {} {} {slot} with {}{} {}",
                c[0],
                c[1],
                c[2],
                it.item,
                super::stack::command_suffix(&it.stack, vec![]),
                it.count
            ));
        }
    }
    out
}

/// The component suffix a v0.8 `collect` stack carries: its `item_name`, or `""`
/// when the objective declares none — a collect stack is a stack, written by
/// the one stack writer ([`super::stack::command_suffix`]).
pub(super) fn item_component_tail(item: &str, name: Option<&str>) -> String {
    let mut s = delvewright_dsl::Stack::of(item);
    s.name = name.map(str::to_string);
    super::stack::command_suffix(&s, vec![])
}

#[cfg(test)]
mod loot_emit_tests {
    use super::*;
    use crate::compiler::plan::{LootItemPlan, LootPlan};

    fn loot_setup_none(loot: &[LootPlan]) -> Vec<String> {
        loot_setup(loot, &[])
    }

    fn item(item: &str, count: u32, name: Option<&str>, ench: &[(&str, u32)]) -> LootItemPlan {
        let mut stack = delvewright_dsl::Stack::counted(item, count);
        stack.name = name.map(str::to_string);
        stack.enchantments = ench.iter().map(|(k, v)| ((*k).to_string(), *v)).collect();
        LootItemPlan { stack, slot: None }
    }

    fn plan_of(items: Vec<LootItemPlan>) -> Vec<LootPlan> {
        vec![LootPlan {
            id: "loot/stores".to_string(),
            anchor: "anchor/stores".to_string(),
            cell: [10, 64, -3],
            items,
            by_effect: false,
        }]
    }

    /// spec-0134 §6: a `by-effect` fill writes nothing in `setup_finish`
    /// and its own `fill_<loot>` function carries exactly the lines the
    /// at-load fill would have; an at-load fill has no function.
    #[test]
    fn a_fill_that_waits_is_its_own_function() {
        let at_load = plan_of(vec![item("minecraft:bread", 3, None, &[])]);
        let at_load_lines = loot_setup_none(&at_load);
        assert_eq!(at_load_lines.len(), 1);
        assert!(loot_fill_fns(&at_load, &[]).is_empty());
        let mut waits = at_load.clone();
        waits[0].by_effect = true;
        assert!(loot_setup_none(&waits).is_empty(), "nothing at load");
        let fns = loot_fill_fns(&waits, &[]);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].0, "fill_stores");
        assert_eq!(fns[0].1, lines(&at_load_lines));
    }

    /// spec-0134 §5: a rocket stack in a fill carries its stars as the
    /// `fireworks` component, through the renderer `give-item` shares.
    #[test]
    fn a_rocket_in_a_fill_carries_its_stars() {
        let mut rocket = item("minecraft:firework_rocket", 2, None, &[]);
        rocket.stack.fireworks = Some(delvewright_dsl::StackFireworks {
            flight: None,
            explosions: vec![delvewright_dsl::FireworkExplosion {
                shape: delvewright_dsl::FireworkShape::Burst,
                colors: vec!["#ff0000".to_string()],
                fade_colors: vec!["#0000ff".to_string()],
                trail: false,
                twinkle: true,
            }],
        });
        let out = loot_setup_none(&plan_of(vec![rocket]));
        assert!(
            out.iter().any(|l| l.ends_with(
                "with minecraft:firework_rocket[fireworks={flight_duration:1b,explosions:[{shape:\"burst\",colors:[I;16711680],fade_colors:[I;255],has_twinkle:1b}]}] 2"
            )),
            "{out:#?}"
        );
    }

    /// Slots are positional and deterministic: nth declared stack -> container.n.
    #[test]
    fn slots_are_assigned_positionally() {
        let out = loot_setup_none(&plan_of(vec![
            item("minecraft:cooked_cod", 3, None, &[]),
            item("minecraft:torch", 16, None, &[]),
        ]));
        assert_eq!(
            out,
            vec![
                "item replace block 10 64 -3 container.0 with minecraft:cooked_cod 3",
                "item replace block 10 64 -3 container.1 with minecraft:torch 16",
            ]
        );
    }

    #[test]
    fn a_named_stack_carries_the_custom_name_component() {
        let out = loot_setup_none(&plan_of(vec![item(
            "minecraft:paper",
            1,
            Some("Tide Ledger"),
            &[],
        )]));
        assert!(
            out[0].contains(r#"custom_name={"italic":false,"text":"Tide Ledger"}"#),
            "{}",
            out[0]
        );
    }

    /// A drop that names itself lowers through [`tr`], never a raw literal.
    ///
    /// The regression this pins: `set_name` used to build `{"text": name}`
    /// directly, and an authored name arrives still carrying its l10n marker —
    /// so a named drop shipped the marker verbatim and `DW0185` refused the
    /// build. Every version, unconditionally, for as long as the surface had
    /// existed; nothing was red because nothing had ever named a drop.
    #[test]
    fn a_named_drop_lowers_through_tr_not_a_raw_literal() {
        let tagged = delvewright_dsl::l10n::tag("wave.muster.mob.0.drop.0.name", "Muster Bone");
        let component = tr(&tagged);
        assert_eq!(
            component["translate"], "wave.muster.mob.0.drop.0.name",
            "a marked string must lower to a translate key: {component}"
        );
        assert_eq!(component["fallback"], "Muster Bone", "{component}");
        assert!(
            component.get("text").is_none(),
            "a marked string must NOT keep a literal body — that body is what \
             carries the marker into the emitted tree: {component}"
        );
        assert!(
            !component.to_string().contains(&tagged),
            "the marker itself must not survive into the component: {component}"
        );
    }

    #[test]
    fn enchantments_emit_as_the_1_21_component_map() {
        let out = loot_setup_none(&plan_of(vec![item(
            "minecraft:iron_sword",
            1,
            None,
            &[("minecraft:sharpness", 3), ("minecraft:knockback", 1)],
        )]));
        // BTreeMap order, never hash order (ADR-0006).
        assert!(
            out[0].contains(r#"enchantments={"minecraft:knockback":1,"minecraft:sharpness":3}"#),
            "{}",
            out[0]
        );
    }

    /// An enchanted book STORES its enchantments, on every surface that writes
    /// a stack: a `loot` fill and an equipped piece ask the same rule a
    /// `give-item` does (`delvewright_dsl::enchantment_component`). Before it,
    /// a book in a chest shipped `enchantments=` — a book that glints and that
    /// an anvil ignores.
    #[test]
    fn an_enchanted_book_stores_its_enchantments_on_every_surface() {
        let out = loot_setup_none(&plan_of(vec![item(
            "minecraft:enchanted_book",
            1,
            None,
            &[("minecraft:mending", 1)],
        )]));
        assert!(
            out[0].contains(
                r#"minecraft:enchanted_book[stored_enchantments={"minecraft:mending":1}] 1"#
            ),
            "{}",
            out[0]
        );
        let mut book = delvewright_dsl::Stack::of("minecraft:enchanted_book");
        book.enchantments.insert("minecraft:mending".to_string(), 1);
        assert_eq!(
            super::super::stack::nbt(&book),
            r#"{id:"minecraft:enchanted_book",count:1,components:{"minecraft:stored_enchantments":{"minecraft:mending":1}}}"#
        );
        let mut sword = book.clone();
        sword.item = "minecraft:iron_sword".to_string();
        assert_eq!(
            super::super::stack::nbt(&sword),
            r#"{id:"minecraft:iron_sword",count:1,components:{"minecraft:enchantments":{"minecraft:mending":1}}}"#
        );
        let tree = crate::compiler::commands::CommandTree::v1_21_11();
        for line in &out {
            assert!(
                tree.validate_line(line).is_ok(),
                "emitted command must validate: {line}\n{:?}",
                tree.validate_line(line)
            );
        }
    }

    /// The emitted fill must be a command 1.21.11 actually accepts — the item
    /// component brackets are new ground here, and a syntax error would only
    /// surface as a silently-skipped line on a live server.
    #[test]
    fn every_emitted_fill_validates_against_the_command_tree() {
        let tree = crate::compiler::commands::CommandTree::v1_21_11();
        let out = loot_setup_none(&plan_of(vec![
            item("minecraft:cooked_cod", 3, None, &[]),
            item("minecraft:paper", 1, Some("Tide Ledger"), &[]),
            item(
                "minecraft:netherite_sword",
                1,
                Some("Bell-Breaker"),
                &[("minecraft:sharpness", 5), ("minecraft:unbreaking", 3)],
            ),
        ]));
        for line in &out {
            assert!(
                tree.validate_line(line).is_ok(),
                "emitted command must validate: {line}\n{:?}",
                tree.validate_line(line)
            );
        }
    }

    #[test]
    fn no_loot_emits_nothing() {
        assert!(loot_setup(&[], &[]).is_empty());
    }

    // --- actor equipment (spec-0021) ---

    fn actor_with(eq: Option<delvewright_dsl::MobEquipment>) -> delvewright_dsl::Actor {
        delvewright_dsl::Actor {
            on_kill: None,
            id: delvewright_dsl::ActorId("actor/elite".to_string()),
            entity: "minecraft:wither_skeleton".to_string(),
            name: None,
            skin: None,
            anchor: delvewright_dsl::AnchorId("anchor/stage".to_string()),
            offset: [0, 0, 0],
            facing: None,
            vulnerable: false,
            equipment: eq,
            attributes: None,
            tier: None,
            drops: Vec::new(),
            traversal: None,
            watch: None,
            answers: None,
            health_bar: None,
            entrance: None,
            outrun: false,
        }
    }

    fn full_kit() -> delvewright_dsl::MobEquipment {
        use delvewright_dsl::EquipItem;
        let enchanted = |item: &str, e: &str, lvl: u32| {
            let mut s = delvewright_dsl::Stack::of(item);
            s.enchantments.insert(e.to_string(), lvl);
            EquipItem::Stack(s)
        };
        delvewright_dsl::MobEquipment {
            head: Some(enchanted(
                "minecraft:netherite_helmet",
                "minecraft:protection",
                4,
            )),
            chest: Some(EquipItem::Plain(
                "minecraft:netherite_chestplate".to_string(),
            )),
            legs: None,
            feet: None,
            main_hand: Some(enchanted(
                "minecraft:netherite_sword",
                "minecraft:sharpness",
                5,
            )),
            off_hand: None,
            body: None,
            saddle: None,
        }
    }

    /// A horse barded and saddled (spec-0067 criterion 7): both new keys ride
    /// the summon's `equipment` compound, each at drop chance 0 unless a drop
    /// names it (criterion 13).
    #[test]
    fn a_barded_and_saddled_horse_carries_body_and_saddle_keys() {
        use delvewright_dsl::EquipItem;
        let mut a = actor_with(Some(delvewright_dsl::MobEquipment {
            head: None,
            chest: None,
            legs: None,
            feet: None,
            main_hand: None,
            off_hand: None,
            body: Some(EquipItem::Plain("minecraft:iron_horse_armor".to_string())),
            saddle: Some(EquipItem::Plain("minecraft:saddle".to_string())),
        }));
        a.entity = "minecraft:horse".to_string();
        for s in [
            actor_puppet_summon("dw", &a, [1, 2, 3], 0),
            actor_twin_summon("dw", &a, "~ ~ ~", TwinArrival::Arrives),
        ] {
            assert!(
                s.contains(
                    "equipment:{body:{id:\"minecraft:iron_horse_armor\",count:1},\
                     saddle:{id:\"minecraft:saddle\",count:1}}"
                ),
                "{s}"
            );
            assert!(s.contains("drop_chances:{body:0.0f,saddle:0.0f}"), "{s}");
        }
        a.tier = Some(delvewright_dsl::EncounterTier::Boss);
        a.drops = vec![delvewright_dsl::MobDrop::Slot(delvewright_dsl::SlotDrop {
            slot: EquipSlot::Saddle,
        })];
        let twin = actor_twin_summon("dw", &a, "~ ~ ~", TwinArrival::Arrives);
        assert!(
            twin.contains(&format!(
                "drop_chances:{{body:0.0f,saddle:{DECLARED_DROP_CHANCE}}}"
            )),
            "a declared saddle drop is guaranteed: {twin}"
        );
    }

    /// The drop-strip line zeroes one `drop_chances` key per slot of
    /// [`EquipSlot::ALL`] (spec-0067 criterion 1).
    #[test]
    fn the_strip_line_zeroes_every_slot_the_game_has() {
        let line = strip_drops_line("dw_actor_x");
        let inner = line
            .split("drop_chances:{")
            .nth(1)
            .and_then(|r| r.split('}').next())
            .expect("the strip line writes a drop_chances compound");
        let keys: Vec<&str> = inner
            .split(',')
            .map(|kv| kv.split(':').next().unwrap())
            .collect();
        assert_eq!(keys.len(), EquipSlot::ALL.len(), "{line}");
        assert_eq!(
            keys,
            EquipSlot::ALL.iter().map(|s| s.nbt()).collect::<Vec<_>>(),
            "{line}"
        );
    }

    /// An actor WITHOUT equipment must emit exactly what it did before the field
    /// existed — including no armed-mob default leaking in from the wave path.
    #[test]
    fn an_unequipped_actor_is_byte_identical() {
        let a = actor_with(None);
        assert_eq!(actor_equipment(&a, &a.entity), None);
        let puppet = actor_puppet_summon("dw", &a, [1, 2, 3], 0);
        assert!(!puppet.contains("equipment:"), "{puppet}");
        assert!(!actor_twin_summon("dw", &a, "~ ~ ~", TwinArrival::Arrives).contains("equipment:"));
    }

    /// The gear rides on BOTH bodies — the dormant puppet and the twin that
    /// replaces it — so unleashing does not undress the elite.
    #[test]
    fn equipment_lands_on_both_the_puppet_and_the_twin() {
        let a = actor_with(Some(full_kit()));
        let puppet = actor_puppet_summon("dw", &a, [1, 2, 3], 0);
        let twin = actor_twin_summon("dw", &a, "~ ~ ~", TwinArrival::Arrives);
        for s in [&puppet, &twin] {
            assert!(
                s.contains(
                    "mainhand:{id:\"minecraft:netherite_sword\",count:1,\
                     components:{\"minecraft:enchantments\":{\"minecraft:sharpness\":5}}}"
                ),
                "enchanted main hand missing:\n{s}"
            );
            assert!(
                s.contains(
                    "head:{id:\"minecraft:netherite_helmet\",count:1,\
                            components:{\"minecraft:enchantments\":{\"minecraft:protection\":4}}}"
                ),
                "enchanted helmet missing:\n{s}"
            );
            assert!(
                s.contains("chest:{id:\"minecraft:netherite_chestplate\",count:1}"),
                "plain chestplate missing:\n{s}"
            );
            // No-grind: an actor's kit is never lootable.
            assert!(
                s.contains("drop_chances:{mainhand:0.0f,head:0.0f,chest:0.0f}"),
                "zero drop chances missing:\n{s}"
            );
            assert!(!s.contains("ArmorItems") && !s.contains("HandItems"));
        }
    }
}
