//! Mob equipment, attributes, enchantments and declared drops.

use super::*;

/// The `,components:{…}` SNBT tail carrying an equipped piece's enchantments,
/// or `""` when it has none (which is what keeps every pre-enchantment campaign
/// byte-identical).
///
/// 1.21 moved enchantments onto the **item component**
/// `minecraft:enchantments`, whose value is a map of enchantment id → level.
/// Emission order is the `BTreeMap`'s id order, never hash order (ADR-0006).
pub(super) fn enchantment_components(piece: &EquipItem) -> String {
    enchantment_component_tail(piece.item(), piece.enchantments())
}

/// The shared `,components:{"minecraft:enchantments":{…}}` renderer — one
/// implementation for equipped gear and for container loot, so the two cannot
/// disagree about the component's shape. Which component is the item's
/// ([`delvewright_dsl::enchantment_component`]): an enchanted book stores.
pub(super) fn enchantment_component_tail(
    item: &str,
    ench: &std::collections::BTreeMap<String, u32>,
) -> String {
    if ench.is_empty() {
        return String::new();
    }
    let body = ench
        .iter()
        .map(|(id, lvl)| format!("\"{id}\":{lvl}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        ",components:{{\"{}\":{{{body}}}}}",
        delvewright_dsl::enchantment_component(item)
    )
}

/// The default main-hand weapon for a summoned mob whose natural spawns are
/// armed, or `None` for mobs that spawn unarmed. Small static table (documented
/// in the compiler README); mobs not listed (zombie, drowned — a wild trident is
/// not a default) get nothing.
/// The pillager entry is load-bearing beyond looks (spec-0016 §6): a pillager's
/// only attack goal is the crossbow goal, so an unarmed one that acquires a
/// target has nothing runnable to do while its patrol goal is blocked by that
/// same target — it freezes in place indefinitely (live-verified on 1.21.11,
/// `docs/notes/td-routing-spike.md`). Arming it by default takes that deadlock
/// off the author's plate entirely; `DW0384` catches the one remaining way in (an
/// explicit `main_hand` override that takes the crossbow away).
pub(super) fn default_mainhand(entity: &str) -> Option<&'static str> {
    match entity.strip_prefix("minecraft:").unwrap_or(entity) {
        "wither_skeleton" => Some("minecraft:stone_sword"),
        "skeleton" | "stray" => Some("minecraft:bow"),
        "pillager" => Some("minecraft:crossbow"),
        "vindicator" => Some("minecraft:iron_axe"),
        _ => None,
    }
}

/// The main-hand item a wave mob **actually spawns holding**: the v0.6
/// `equipment.main_hand` override when the author gave one, else the
/// armed-mob default table ([`default_mainhand`]).
///
/// This is the single source of truth for "what is in this mob's hand" and
/// must be used by anything that *describes* the emitted summon — notably the
/// generated `verb_kill` PackTest arming assertion. Reading the default table
/// there instead produced a self-contradicting datapack: the summon gave the
/// override (the-drowned-bell's vindicators carry `minecraft:stone_axe`) while
/// the generated test asserted the default (`minecraft:iron_axe`), so the suite
/// failed on a real server for a campaign that was in fact correct.
pub(super) fn effective_mainhand<'a>(
    entity: &str,
    eq: Option<&'a MobEquipment>,
) -> Option<&'a str> {
    eq.and_then(|e| e.main_hand.as_ref())
        .map(|p| p.item())
        .or_else(|| default_mainhand(entity))
}

/// Default hand equipment for a summoned mob whose natural spawns are armed
/// (M2 fix 5). `/summon` gives no equipment, so a wither-skeleton boss spawned
/// unarmed was trivial. Returns an SNBT fragment (no leading comma) setting the
/// `equipment` component with a zero `drop_chances`, or `None` for unarmed mobs.
///
/// **Component-era form, not legacy `HandItems` (M2 round-2 fix 1).** Minecraft
/// 1.21.11 silently ignores `HandItems`/`HandDropChances` on `/summon` NBT — a
/// `data get entity … HandItems` after summon returns nothing and the mob is
/// bare-handed. The accepted form is the entity `equipment`/`drop_chances`
/// components: proven live via rcon (`equipment:{mainhand:{id:"minecraft:
/// stone_sword",count:1}},drop_chances:{mainhand:0.0f}` → `data get entity …
/// equipment.mainhand` returns the item; the legacy form yields "Found no
/// elements matching equipment"). The legacy form failed *silently* for a whole
/// milestone because nothing looked — the generated `verb_kill` PackTest now
/// asserts the armed mob actually holds its weapon so a regression can't hide.
pub(super) fn default_equipment(entity: &str) -> Option<String> {
    default_mainhand(entity).map(|item| {
        format!("equipment:{{mainhand:{{id:\"{item}\",count:1}}}},drop_chances:{{mainhand:0.0f}}")
    })
}

/// The drop chance a **declared** drop puts on its slot (DSL v0.9).
///
/// Not `1.0`. Vanilla's `DropChances` record (pinned 1.21.11 client jar, class
/// `cgi`) has exactly two named operations here, and they say what the numbers
/// mean:
///
/// * `withGuaranteedDrop(slot)` writes the constant `2.0f` — verified in the
///   jar's bytecode (`fconst_2`), and the same value the vanilla
///   `SaddleEquipmentSlotFix` datafixer writes for a saddle a horse always
///   drops;
/// * `isPreserved(slot)` is `chance > 1.0f`.
///
/// `Mob.dropCustomDeathLoot` (class `chn`) reads both: a slot whose chance is
/// exactly `0.0f` is skipped outright; a **preserved** slot drops even when the
/// killing blow was not a player's, and — the reason `1.0f` is wrong — it skips
/// the durability randomization that a chance of `≤ 1.0` applies to a damageable
/// item. A boss axe declared as a drop must be *the* axe, not a die-roll of its
/// remaining durability: `2.0f` is the vanilla primitive for "always, unchanged",
/// and it is the only value that makes a declared drop deterministic.
pub(super) const DECLARED_DROP_CHANCE: &str = "2.0f";

/// The drop chance every UNDECLARED slot keeps — today's behaviour, unchanged,
/// which is what keeps every pre-0.9 campaign byte-identical.
pub(super) const NO_DROP_CHANCE: &str = "0.0f";

/// The vanilla NBT slot keys a `drops[]` list marks as guaranteed, for one
/// entity. Quest-item entries carry no slot and are absent from the set — they
/// ride the death loot table instead ([`drop_loot_table`]).
pub(super) fn declared_drop_slots(drops: &[delvewright_dsl::MobDrop]) -> BTreeSet<&'static str> {
    drops
        .iter()
        .filter_map(|d| d.slot())
        .map(|s| s.nbt())
        .collect()
}

/// The chance string for `slot`, given the entity's declared drops.
pub(super) fn drop_chance_for(slot: &str, declared: &BTreeSet<&'static str>) -> &'static str {
    if declared.contains(slot) {
        DECLARED_DROP_CHANCE
    } else {
        NO_DROP_CHANCE
    }
}

/// The datapack path (namespace-local) of the death loot table a declared
/// quest-item drop rides on, for one actor / one wave-mob stack.
///
/// **Why a loot table and not another equipment slot.** The `equipment` /
/// `drop_chances` compounds address the six worn slots and nothing else — a
/// quest token the fight *yields* has no slot, and hanging it in an off-hand the
/// author never dressed would be the downstream workaround the no-hack rule
/// forbids. 1.21.11 answers the slot-less half with its own primitive: `Mob`
/// (jar class `chn`) reads `DeathLootTable` (and `DeathLootTableSeed`) straight
/// off summon NBT, through the `ResourceKey<LootTable>` codec, and
/// `LivingEntity.dropAllDeathLoot` rolls it on death. The compiler already
/// writes `DeathLootTable:"minecraft:empty"` on every actor; a declared drop
/// simply points the same field at a table the compiler emits, with the item
/// entry the author declared. One roll, one entry, no RNG (ADR-0006).
pub(super) fn drop_loot_path(kind: &str, id: &str) -> String {
    format!("dw_drop/{kind}_{}", plan::safe_local(id))
}

/// The `DeathLootTable` NBT value for an entity: the emitted table when it
/// declares a quest-item drop, else the `minecraft:empty` every actor has always
/// carried (byte-identity for every pre-0.9 campaign).
pub(super) fn death_loot_table(ns: &str, path: Option<String>) -> String {
    match path {
        Some(p) => format!("{ns}:{p}"),
        None => "minecraft:empty".to_string(),
    }
}

/// True if this drop list contains a quest-item entry (the half that needs a
/// death loot table).
pub(super) fn has_item_drop(drops: &[delvewright_dsl::MobDrop]) -> bool {
    drops.iter().any(|d| d.item().is_some())
}

/// Strip a declared drop off a body the **compiler** is about to remove.
///
/// The invariant, stated once: a declared drop is what a *player's kill* yields.
/// Every removal the compiler performs itself — the `unleash` that swaps a
/// puppet for its twin, a `despawn-actor` (either style), a bonfire's re-seat of
/// a wave (`wave_reseat_<wave>`, both the `respawns_on_rest` and the undefeated
/// billed kind) and of an unleashed actor (`actor_restand_<id>`) — is built by
/// [`removal_lines`] and ends in `/kill`, in place or under the world for
/// [`Exit::Unseen`], and vanilla `/kill` is an ordinary death:
/// a preserved slot (chance > 1.0) drops **even when the killer is not a
/// player**. Without this line an elite would shed its axe every time the story
/// moved it, and a re-seat would turn the boss into a vending machine.
///
/// Two intended vanilla primitives, composed: `execute as … run data merge
/// entity @s` (single-entity by construction, which is what `data merge`
/// requires) writing drop chance 0 on every slot and an empty death loot table.
/// Emitted only for an actor that declares drops. One `drop_chances` key per
/// slot of [`EquipSlot::ALL`], so a slot the DSL gains is stripped with no edit
/// here.
pub(super) fn strip_drops_line(tag: &str) -> String {
    let zeros: Vec<String> = EquipSlot::ALL
        .iter()
        .map(|s| format!("{}:{NO_DROP_CHANCE}", s.nbt()))
        .collect();
    format!(
        "execute as @e[tag={tag}] run data merge entity @s {{drop_chances:{{{}}},DeathLootTable:\"minecraft:empty\"}}",
        zeros.join(",")
    )
}

/// Whether any mob of this wave declares a drop — the wave's bodies share one
/// tag, so the removal strips them all when one of them carries loot.
pub(super) fn wave_declares_drops(w: &delvewright_dsl::Wave) -> bool {
    w.mobs.iter().any(|m| !m.drops.is_empty())
}

/// Whether this actor declares a drop (on either body: puppet or twin).
pub(super) fn actor_declares_drops(a: &delvewright_dsl::Actor) -> bool {
    !a.drops.is_empty()
}

/// The `equipment`/`drop_chances` SNBT fragment for a wave mob (no leading
/// comma), or `None` for a bare-handed mob. A mob without the v0.6 `equipment`
/// field takes the [`default_equipment`] path **unchanged** (byte-identity for
/// pre-equipment waves). With the field, explicit slots merge over the
/// armed-mob main-hand default (an explicit `main_hand` overrides it — a
/// helmeted skeleton keeps its bow). Every slot the v0.9 `drops[]` list does not
/// name carries drop chance 0: players must never farm wave gear (no-grind
/// constitution); a named slot carries [`DECLARED_DROP_CHANCE`]. Component-era
/// form only — see [`default_equipment`] for why legacy `ArmorItems`/
/// `HandItems` are silently ignored by 1.21.11 `/summon`. Slot order is
/// [`EquipSlot::ALL`]'s, fixed for ADR-0006 determinism.
/// Which slot holds which item on a summoned wave mob, and the authored piece
/// behind it where there is one — the ONE resolution of "what does this stack
/// actually wear".
///
/// Two readers, and they must never disagree: [`wave_equipment`] writes the
/// `summon` NBT from it, and [`crate::compiler::muster`] turns it into the
/// questions the live body is asked. A second derivation here would be a probe
/// that verifies its own copy of the declaration instead of the emitted one.
///
/// The main-hand slot is the one place a DEFAULT (a bare id, no enchantments) can
/// stand in for an authored piece, so it carries an id plus an optional authored
/// piece; every other slot is authored or absent.
pub(crate) fn wave_equipment_slots<'a>(
    entity: &str,
    eq: Option<&'a MobEquipment>,
) -> Vec<(EquipSlot, &'a str, Option<&'a EquipItem>)> {
    let Some(eq) = eq else {
        return default_mainhand(entity)
            .map(|it| vec![(EquipSlot::MainHand, it, None)])
            .unwrap_or_default();
    };
    let mainhand = effective_mainhand(entity, Some(eq));
    let mut out = Vec::new();
    for (slot, piece) in eq.pieces() {
        let item = if slot == EquipSlot::MainHand {
            mainhand
        } else {
            piece.map(EquipItem::item)
        };
        if let Some(it) = item {
            out.push((slot, it, piece));
        }
    }
    out
}

pub(super) fn wave_equipment(
    entity: &str,
    eq: Option<&MobEquipment>,
    drops: &[delvewright_dsl::MobDrop],
) -> Option<String> {
    let declared = declared_drop_slots(drops);
    // A stack with no `equipment` field keeps the pre-v0.6 default path exactly:
    // the armed-mob main-hand at drop chance 0, whatever `drops[]` says. Byte
    // identity for every wave that predates the field.
    if eq.is_none() {
        return default_equipment(entity);
    }
    let mut items: Vec<String> = Vec::new();
    let mut chances: Vec<String> = Vec::new();
    for (slot, it, piece) in wave_equipment_slots(entity, eq) {
        let key = slot.nbt();
        let comps = piece.map(enchantment_components).unwrap_or_default();
        items.push(format!("{key}:{{id:\"{it}\",count:1{comps}}}"));
        chances.push(format!("{key}:{}", drop_chance_for(key, &declared)));
    }
    if items.is_empty() {
        return None;
    }
    Some(format!(
        "equipment:{{{}}},drop_chances:{{{}}}",
        items.join(","),
        chances.join(",")
    ))
}

/// The `,attributes:[…]` SNBT fragment (leading comma) for a wave mob's v0.4
/// attribute overrides, or `""` when none are set. Each present field becomes a
/// `{id:"minecraft:<attr>",base:<double>}` entry; doubles are formatted with a
/// decimal point so SNBT reads them as doubles (ADR-0006 determinism).
pub(super) fn attributes_snbt(attrs: Option<&delvewright_dsl::MobAttributes>) -> String {
    wrap_attribute_entries(attribute_entries(attrs))
}

/// The individual `{id:…,base:…}` entries for a [`MobAttributes`] block, in the
/// fixed schema order. Split out of [`attributes_snbt`] so the paths that add a
/// compiler-owned attribute of their own (the `vulnerable` actor's
/// knockback-immunity) can concatenate rather than fork the table — the DSL
/// exposes ONE attribute surface and there is one place that renders it.
///
/// [`MobAttributes`]: delvewright_dsl::MobAttributes
pub(super) fn attribute_entries(attrs: Option<&delvewright_dsl::MobAttributes>) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    let Some(a) = attrs else {
        return entries;
    };
    let mut add = |id: &str, v: Option<f64>| {
        if let Some(x) = v {
            entries.push(format!("{{id:\"minecraft:{id}\",base:{}}}", fmt_f64(x)));
        }
    };
    add("max_health", a.max_health);
    add("attack_damage", a.attack_damage);
    add("movement_speed", a.movement_speed);
    add("follow_range", a.follow_range);
    entries
}

/// Wrap rendered attribute entries as the `,attributes:[…]` SNBT fragment
/// (leading comma), or `""` when there are none.
pub(super) fn wrap_attribute_entries(entries: Vec<String>) -> String {
    if entries.is_empty() {
        String::new()
    } else {
        format!(",attributes:[{}]", entries.join(","))
    }
}

/// The **death loot tables** a campaign's declared quest-item drops need (DSL
/// v0.9), as `(namespace-local path, json)` pairs.
///
/// One table per declaring body, one pool, one roll, one entry per declared
/// item: nothing here rolls a die. The entry is the vanilla `minecraft:item`
/// form, and a declared display `name` becomes the `minecraft:set_name` function
/// with `target: "custom_name"` — the same component a `collect`'s `item_name`
/// writes into a container stack, so the key a boss leaves on the ground and the
/// key a barrel hands over are the same item.
///
/// Emitted only for bodies that declare an `{item}` drop, so a campaign without
/// one writes no `loot_table` directory at all and stays byte-identical.
pub(super) fn emit_drop_loot_tables(plan: &Plan) -> Vec<(String, Value)> {
    let c = plan.campaign;
    let mut out: Vec<(String, Value)> = Vec::new();
    let table = |drops: &[delvewright_dsl::MobDrop]| {
        let entries: Vec<Value> = drops
            .iter()
            .filter_map(|d| {
                let item = d.item()?;
                let mut entry = json!({ "type": "minecraft:item", "name": item });
                if let Some(name) = d.name() {
                    entry["functions"] = json!([{
                        "function": "minecraft:set_name",
                        "target": "custom_name",
                        // `tr`, not a bare `{"text": …}`. An authored display
                        // name arrives here still carrying its l10n marker, and
                        // a raw text component ships the marker verbatim — which
                        // `DW0185` refuses, so a drop that named itself did not
                        // build AT ALL, at any version. The diagnostic was right
                        // and nothing had ever reached it: no campaign and no
                        // fixture had named a drop, which is the coverage gap
                        // spec-0039 exists to close rather than a missing rule.
                        "name": tr(name),
                    }]);
                }
                Some(entry)
            })
            .collect();
        json!({
            "type": "minecraft:entity",
            "pools": [{ "rolls": 1, "entries": entries }],
        })
    };
    for a in &c.quests.content.actors {
        if has_item_drop(&a.drops) {
            out.push((drop_loot_path("actor", a.id.as_str()), table(&a.drops)));
        }
    }
    for w in &c.quests.content.waves {
        for (k, m) in w.mobs.iter().enumerate() {
            if has_item_drop(&m.drops) {
                out.push((
                    drop_loot_path("wave", &format!("{}-{k}", w.id.as_str())),
                    table(&m.drops),
                ));
            }
        }
    }
    out
}
