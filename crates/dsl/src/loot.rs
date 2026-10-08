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
