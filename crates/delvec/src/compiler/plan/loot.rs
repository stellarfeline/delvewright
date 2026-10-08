//! Loot: containers, their items, and the `collect` fills.

use super::*;

/// Resolve every stage-5 trap (DSL v0.6, spec-0011) in content order into a
/// [`TrapPlan`]: the trigger/hazard cell (the trap's `at` anchor), the dispenser
/// socket cell (from the `at` anchor's metadata), the dispense payload, and the
/// disarm affordance. A trap whose `at` anchor does not resolve to a point is
/// skipped (validation guarantees the anchor exists; an unresolved pool anchor
/// simply carries no proof/emission — the same policy as `collect_v06_effects`).
/// A resolved container fill (spec-0021).
#[derive(Clone, Debug, PartialEq)]
pub struct LootPlan {
    /// Loot id (`loot/<kebab>`).
    pub id: String,
    /// The anchor named by the declaration.
    pub anchor: String,
    /// The world cell of the container to fill.
    pub cell: [i32; 3],
    /// Contents in declaration order; index IS the container slot.
    pub items: Vec<LootItemPlan>,
}

/// One stack in a [`LootPlan`].
#[derive(Clone, Debug, PartialEq)]
pub struct LootItemPlan {
    /// Item id.
    pub item: String,
    /// Stack size.
    pub count: u32,
    /// Custom name, already localized by the build language.
    pub name: Option<String>,
    /// Enchantment id → level.
    pub enchantments: BTreeMap<String, u32>,
}

/// A `collect` objective that ADOPTS a prefab-placed container (DSL v0.8),
/// resolved to the container's world cell.
///
/// One resolution, one cell: the build-tier container proof (`DW0438`), the
/// activation-time fill and the critical-path step the bot opens all read THIS
/// value, so the cell the compiler proves is provably the cell it fills and the
/// cell the bot walks to. Resolving the anchor separately at each site is how a
/// proof and its emission drift apart.
#[derive(Clone, Debug, PartialEq)]
pub struct CollectFillPlan {
    /// The `collect` objective's id.
    pub objective_id: String,
    /// The anchor named by `container`.
    pub anchor: String,
    /// The world cell of the container to fill.
    pub cell: [i32; 3],
    /// How many slots the fill occupies: the objective's own stack plus
    /// `fill_count` padding stacks.
    pub slots: usize,
}

/// Resolve every `collect` objective's adopted `container` (DSL v0.8) to a world
/// cell, in campaign order. An unresolvable anchor is skipped here and reported
/// by the DSL tier (`DW0142`) — the same policy [`collect_loot`] follows.
pub(super) fn collect_collect_fills(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<CollectFillPlan> {
    let mut out = Vec::new();
    for q in &campaign.quests.content.quests {
        for o in &q.objectives {
            let Some(cont) = o.collect_container() else {
                continue;
            };
            let Some(cell) = point_any(anchors, cont.as_str()) else {
                continue;
            };
            out.push(CollectFillPlan {
                objective_id: o.id().as_str().to_string(),
                anchor: cont.as_str().to_string(),
                cell,
                slots: 1 + o.collect_fill_count() as usize,
            });
        }
    }
    out
}

/// Resolve every stage-5 `loot` declaration to a world cell. An unresolvable
/// anchor is skipped here and reported by the DSL tier (`DW0142`).
pub(super) fn collect_loot(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<LootPlan> {
    campaign
        .quests
        .content
        .loot
        .iter()
        .filter_map(|l| {
            let cell = point_any(anchors, l.anchor.as_str())?;
            Some(LootPlan {
                id: l.id.as_str().to_string(),
                anchor: l.anchor.as_str().to_string(),
                cell,
                items: l
                    .items
                    .iter()
                    .map(|it| LootItemPlan {
                        item: it.item.clone(),
                        count: it.count,
                        name: it.name.clone(),
                        enchantments: it.enchantments.clone(),
                    })
                    .collect(),
            })
        })
        .collect()
}
