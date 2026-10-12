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
pub(super) fn loot_setup(loot: &[crate::compiler::plan::LootPlan]) -> Vec<String> {
    let mut out = Vec::new();
    for l in loot {
        let c = l.cell;
        for (slot, it) in l.items.iter().enumerate() {
            out.push(format!(
                "item replace block {} {} {} container.{slot} with {}{} {}",
                c[0],
                c[1],
                c[2],
                it.item,
                container_stack_components(&it.item, it.name.as_deref(), &it.enchantments),
                it.count
            ));
        }
    }
    out
}

/// The `[custom_name=…,enchantments=…]` component suffix a container-fill stack
/// carries in `item replace … with <item><suffix> <count>`, or `""` when it
/// carries neither — which is what keeps every unnamed, unenchanted fill
/// byte-identical to the emission that predates both fields.
///
/// ONE renderer for every container fill: spec-0021 `loot` and the DSL v0.8
/// `collect` `item_name`. A quest item named on one surface and
/// unnamed on the other would be the same defect the wave-arming table taught —
/// two places describing one stack, drifting apart the moment either moves.
/// Enchantment order is the `BTreeMap`'s id order, never hash order (ADR-0006).
pub(super) fn container_stack_components(
    item: &str,
    name: Option<&str>,
    ench: &std::collections::BTreeMap<String, u32>,
) -> String {
    let mut comps: Vec<String> = Vec::new();
    if let Some(n) = name {
        comps.push(format!(
            "custom_name={}",
            tr_with(n, &[("italic", json!(false))])
        ));
    }
    if !ench.is_empty() {
        let body = ench
            .iter()
            .map(|(id, lvl)| format!("\"{id}\":{lvl}"))
            .collect::<Vec<_>>()
            .join(",");
        // The component the item writes them to: an enchanted book stores them.
        let comp = delvewright_dsl::enchantment_component(item);
        comps.push(format!(
            "{}={{{body}}}",
            comp.strip_prefix("minecraft:").unwrap_or(comp)
        ));
    }
    if comps.is_empty() {
        return String::new();
    }
    format!("[{}]", comps.join(","))
}

/// The component suffix a v0.8 `collect` stack carries: its `item_name`, or `""`
/// when the objective declares none. A thin alias over
/// [`container_stack_components`] — a collect stack is a container fill, and is
/// rendered by the container fill's renderer.
pub(super) fn item_component_tail(item: &str, name: Option<&str>) -> String {
    container_stack_components(item, name, &std::collections::BTreeMap::new())
}

#[cfg(test)]
mod loot_emit_tests {
    use super::*;
    use crate::compiler::plan::{LootItemPlan, LootPlan};

    fn item(item: &str, count: u32, name: Option<&str>, ench: &[(&str, u32)]) -> LootItemPlan {
        LootItemPlan {
            item: item.to_string(),
            count,
            name: name.map(str::to_string),
            enchantments: ench.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
        }
    }

    fn plan_of(items: Vec<LootItemPlan>) -> Vec<LootPlan> {
        vec![LootPlan {
            id: "loot/stores".to_string(),
            anchor: "anchor/stores".to_string(),
            cell: [10, 64, -3],
            items,
        }]
    }

    /// Slots are positional and deterministic: nth declared stack -> container.n.
    #[test]
    fn slots_are_assigned_positionally() {
        let out = loot_setup(&plan_of(vec![
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
        let out = loot_setup(&plan_of(vec![item(
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
        let out = loot_setup(&plan_of(vec![item(
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
        let out = loot_setup(&plan_of(vec![item(
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
        let mut ench = std::collections::BTreeMap::new();
        ench.insert("minecraft:mending".to_string(), 1);
        assert_eq!(
            enchantment_component_tail("minecraft:enchanted_book", &ench),
            r#",components:{"minecraft:stored_enchantments":{"minecraft:mending":1}}"#
        );
        assert_eq!(
            enchantment_component_tail("minecraft:iron_sword", &ench),
            r#",components:{"minecraft:enchantments":{"minecraft:mending":1}}"#
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
        let out = loot_setup(&plan_of(vec![
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
        assert!(loot_setup(&[]).is_empty());
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
            health_bar: None,
        }
    }

    fn full_kit() -> delvewright_dsl::MobEquipment {
        use delvewright_dsl::{EnchantedItem, EquipItem};
        delvewright_dsl::MobEquipment {
            head: Some(EquipItem::Enchanted(EnchantedItem {
                item: "minecraft:netherite_helmet".to_string(),
                enchantments: [("minecraft:protection".to_string(), 4)]
                    .into_iter()
                    .collect(),
            })),
            chest: Some(EquipItem::Plain(
                "minecraft:netherite_chestplate".to_string(),
            )),
            legs: None,
            feet: None,
            main_hand: Some(EquipItem::Enchanted(EnchantedItem {
                item: "minecraft:netherite_sword".to_string(),
                enchantments: [("minecraft:sharpness".to_string(), 5)]
                    .into_iter()
                    .collect(),
            })),
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
            actor_twin_summon("dw", &a, "~ ~ ~"),
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
        let twin = actor_twin_summon("dw", &a, "~ ~ ~");
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
        assert!(!actor_twin_summon("dw", &a, "~ ~ ~").contains("equipment:"));
    }

    /// The gear rides on BOTH bodies — the dormant puppet and the twin that
    /// replaces it — so unleashing does not undress the elite.
    #[test]
    fn equipment_lands_on_both_the_puppet_and_the_twin() {
        let a = actor_with(Some(full_kit()));
        let puppet = actor_puppet_summon("dw", &a, [1, 2, 3], 0);
        let twin = actor_twin_summon("dw", &a, "~ ~ ~");
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
