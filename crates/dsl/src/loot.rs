//! Loot containers and what they hold.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AnchorId, LootId};

#[cfg(doc)]
use crate::enchantment_component;

/// A stage-5 container fill (DSL v0.6, spec-0021): contents for a chest or
/// barrel the prefab already placed.
///
/// The container is **hardware the prefab authored**, exactly like a trap's
/// dispenser: this declaration gives an already-placed, already-lit, already
/// composed piece of furniture its contents. The compiler never places the
/// container itself — if the anchor's cell does not already hold one, that is a
/// content defect and a build error (`DW0431`), not something to paper over by
/// setblock-ing a chest into a wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Loot {
    /// Unique loot id (`loot/<kebab>`).
    pub id: LootId,
    /// The anchor whose cell holds the container to fill.
    pub anchor: AnchorId,
    /// Contents, in declaration order. Slot assignment is positional and
    /// deterministic — the first entry lands in `container.0`, the second in
    /// `container.1`, and so on (ADR-0006: no RNG, no loot tables).
    pub items: Vec<LootItem>,
}

/// One stack inside a [`Loot`] container.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LootItem {
    /// Item id (e.g. `minecraft:cooked_cod`). Validated against the pinned
    /// 1.21.11 item registry (`DW0143`).
    pub item: String,
    /// Stack size. Defaults to 1.
    #[serde(default = "one_u32")]
    pub count: u32,
    /// Optional custom item name. Enters the l10n string inventory exactly like
    /// a class kit item's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Enchantments on this stack (`{"minecraft:sharpness": 3}`), emitted as the
    /// 1.21 `minecraft:enchantments` item component — or, on a
    /// `minecraft:enchanted_book`, `minecraft:stored_enchantments`
    /// ([`enchantment_component`]).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub enchantments: BTreeMap<String, u32>,
}

fn one_u32() -> u32 {
    1
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::registry::{AnchorRegistry, ItemRegistry};
use crate::validate::{AnchorProviders, station_kind_diag};
use crate::wave::check_enchantments;

/// Exclusive ownership of an adopted container (DSL v0.8, `DW0435`).
///
/// Both container-fill surfaces write **positionally** from `container.0`: a
/// `loot` entry and a `collect`'s adopted container filling one cell overwrite
/// each other slot-for-slot, and the loser vanishes without a word — the same
/// silent-overwrite defect `DW0435` already names for two `loot` entries, reached
/// through a second door. Two `collect` objectives sharing a container is the
/// same collision (and worse: whichever activates second replaces the first
/// objective's items with its own).
///
/// Only claims involving at least one `collect` are reported here; `loot`-vs-
/// `loot` stays in [`loot_checks`], so nothing is diagnosed twice.
pub(crate) fn collect_container_claim_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    // (anchor -> what claims it), in declaration order. `loot` first: a loot
    // entry is the surface that exists to fill a container, so it reads as the
    // incumbent in the message.
    let mut claimed: BTreeMap<&str, String> = BTreeMap::new();
    for l in &c.quests.content.loot {
        claimed
            .entry(l.anchor.as_str())
            .or_insert_with(|| format!("loot `{}`", l.id));
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for (j, o) in q.objectives.iter().enumerate() {
            let Some(cont) = o.collect_container() else {
                continue;
            };
            let mine = format!("collect objective `{}`", o.id());
            if let Some(prev) = claimed.get(cont.as_str()) {
                d.push(Diagnostic::error(
                    codes::LOOT_DUPLICATE_ANCHOR,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/container"),
                    format!(
                        "{prev} and {mine} both fill the container at anchor `{cont}`. Slots \
                         are assigned positionally from `container.0`, so one fill overwrites \
                         the other slot-for-slot and its items never reach the player. Give the \
                         collect its own container anchor (prefabs may expose several), or \
                         fold the other fill's items into it — do NOT rely on declaration \
                         order to combine them."
                    ),
                ));
            } else {
                claimed.insert(cont.as_str(), mine);
            }
        }
    }
}

/// The smallest vanilla container the container-fill surfaces admit. A barrel and
/// a single chest both hold 27; refusing >27 up front keeps the overflow from
/// being discovered as a silently dropped stack on a live server. Shared by the
/// `loot` stack ceiling and the v0.8 `collect` `fill_count` ceiling, which are the
/// same positional-fill rule (`DW0432`) on two surfaces.
pub(crate) const MIN_CONTAINER_SLOTS: usize = 27;

/// `DW0436`: a **single-slot fill** whose `count` exceeds the item's
/// `minecraft:max_stack_size` in the pinned 1.21.11 registry.
///
/// Every one of these compiles to `item replace … container.<n> with <item>
/// <count>`, and that command fails **silently** above the cap: the slot simply
/// stays empty and the server logs nothing. A `count: 2` of `minecraft:rabbit_stew`
/// (cap 1) shipped an empty chest slot in the-drowned-bell round 2 — exactly the
/// silent-failure class `DW0431` exists for, one tier too late. The cap comes from
/// Mojang's own item-components data, vendored per MC pin
/// (`crates/delvec/data/item-stack-sizes-1.21.11.json`), never a hand table.
///
/// Skipped when the registry does not carry stack sizes (the small vendored DSL-side
/// subset) or the item id is unknown — the latter is already `DW0143`, and stacking
/// a second diagnostic on one typo is noise.
pub(crate) fn check_stack_count(
    item: &str,
    count: u32,
    what: &str,
    path: String,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let Some(cap) = items.max_stack_size(item) else {
        return;
    };
    if count <= cap {
        return;
    }
    d.push(Diagnostic::error(
        codes::ITEM_COUNT_OVER_STACK,
        "quests",
        path,
        format!(
            "{what} declares `{item}` × {count}, but `{item}` stacks to at most {cap} in \
             1.21.11. This is filled with `item replace … container.<n>`, which fails \
             SILENTLY above the cap — the slot ships empty and nothing is logged. Lower \
             the count to {cap} or fewer, or declare additional entries/containers."
        ),
    ));
}

/// Stage-5 `loot` declarations (spec-0021): id syntax/uniqueness (`DW0110`/
/// `DW0111`), anchor resolution (`DW0142`), item ids (`DW0143`), enchantments
/// (`DW0433`/`DW0434`), duplicate anchors (`DW0435`) and slot overflow
/// (`DW0432`).
///
/// The *container-ness* of the anchor's cell is deliberately NOT checked here:
/// it needs the assembled world, so it is a build-tier proof (`DW0431`) in the
/// compiler. This tier checks everything decidable from the DSL alone.
pub(crate) fn loot_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    if quests.loot.is_empty() {
        return;
    }
    let ench_reg = crate::registry::VendoredEnchantmentRegistry::v1_21_11();

    let providers = AnchorProviders::build(c, anchors);

    let mut seen_id: BTreeSet<&str> = BTreeSet::new();
    let mut seen_anchor: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, l) in quests.loot.iter().enumerate() {
        if !l.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/loot/{i}/id"),
                format!(
                    "malformed loot id `{}` — loot ids must be lowercase kebab-case with the \
                     `loot/` prefix (e.g. `loot/galley-stores`)",
                    l.id
                ),
            ));
        }
        if !seen_id.insert(l.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/loot/{i}/id"),
                format!("duplicate loot id `{}`", l.id),
            ));
        }
        if let Some(f) = station_kind_diag(
            &providers,
            l.anchor.as_str(),
            crate::layout::StationKind::Point,
            "a loot chest",
            "quests",
            format!("/content/loot/{i}/anchor"),
        ) {
            d.push(f);
        }
        if !providers.resolvable(l.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/loot/{i}/anchor"),
                format!(
                    "loot anchor `{}` is not provided by any prefab bound in this campaign — {}",
                    l.anchor,
                    providers.anchor_remedy(
                        "use an anchor the prefab exposes (anchor names come from prefab \
                         metadata; do NOT invent one)"
                    ),
                ),
            ));
        }
        // Two fills on one container: the second `item replace block` overwrites
        // the first slot-for-slot, so one declaration silently loses.
        if let Some(prev) = seen_anchor.insert(l.anchor.as_str(), i) {
            d.push(Diagnostic::error(
                codes::LOOT_DUPLICATE_ANCHOR,
                "quests",
                format!("/content/loot/{i}/anchor"),
                format!(
                    "loot `{}` and loot `{}` both fill anchor `{}`. Slots are assigned \
                     positionally from `container.0`, so the later declaration overwrites the \
                     earlier one and its items never appear. Merge the two `items` lists into \
                     ONE `loot` entry — do NOT rely on declaration order to combine them.",
                    quests.loot[prev].id, l.id, l.anchor
                ),
            ));
        }
        if l.items.len() > MIN_CONTAINER_SLOTS {
            d.push(Diagnostic::error(
                codes::LOOT_TOO_MANY_ITEMS,
                "quests",
                format!("/content/loot/{i}/items"),
                format!(
                    "loot `{}` declares {} stacks, more than the {MIN_CONTAINER_SLOTS} slots a \
                     vanilla chest or barrel has. Slots are assigned positionally, so every \
                     stack past the {MIN_CONTAINER_SLOTS}th would be dropped silently. Split \
                     the contents across more than one container.",
                    l.id,
                    l.items.len()
                ),
            ));
        }
        for (k, it) in l.items.iter().enumerate() {
            if !items.contains(&it.item) {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    "quests",
                    format!("/content/loot/{i}/items/{k}/item"),
                    format!(
                        "loot item `{}` is not in the pinned 1.21.11 item registry — use a \
                         valid namespaced item id (e.g. `minecraft:cooked_cod`)",
                        it.item
                    ),
                ));
            }
            check_stack_count(
                &it.item,
                it.count,
                &format!("loot `{}`", l.id),
                format!("/content/loot/{i}/items/{k}/count"),
                items,
                d,
            );
            check_enchantments(
                &it.enchantments,
                &format!("loot `{}` item `{}`", l.id, it.item),
                "quests",
                &format!("/content/loot/{i}/items/{k}/enchantments"),
                &ench_reg,
                d,
            );
        }
    }
}
