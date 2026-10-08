//! Stage 5 — trade and the recovery stake (DSL v0.10, spec-0032).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AnchorId, FlagId, QuestEffect, ShopId, StakeId, StateCompare, StateId};

/// A stage-5 **shop** (DSL v0.10, spec-0032): an interaction point that opens a
/// list of offers, each of which is a gate and a bundle of effects.
///
/// # It is the rest flow with different buttons, and that is the whole design
///
/// A bonfire is already an interaction entity, a player-interaction advancement
/// that supplies the acting player, a `minecraft:multi_action` dialog, buttons
/// that run `/trigger`, and tick dispatch. A shop is that same hardware with an
/// authored button list, so nothing here is new machinery — it is the machinery
/// spec-0016 §1 shipped, made author-visible.
///
/// # There is no price field, and that is deliberate
///
/// A price is *"may this happen yet?"*, which is the question
/// [`Gate`](crate::gate::Gate) answers for six other object classes. An offer is
/// therefore the **seventh gate consumer**: it carries `requires_flags`,
/// `forbids_flags` and `requires_state`, and the numeric comparison it needs is
/// the one spec-0031 put in the gate rather than in the verb that first asked.
/// A `price` field would be a second comparison surface for one meaning, which is
/// the defect CLAUDE.md names first.
///
/// # Villager `Offers` is excluded
///
/// Three independent reasons, any one sufficient (spec-0032): a vanilla trade
/// cost can only ever be an item, never a scoreboard value; right-click on a
/// villager body is already allocated to dialogue; and the data-driven trade
/// registry post-dates the pinned 1.21.11.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Shop {
    /// Unique shop id (`shop/<kebab>`).
    pub id: ShopId,
    /// The prefab anchor the shop's interaction point stands on.
    pub anchor: AnchorId,
    /// The dialog's title line. Player-visible, inventoried under
    /// `shop.<id>.title`.
    pub title: String,
    /// The item the visible marker renders as (default `minecraft:emerald`).
    /// Cosmetic only: the marker is a `minecraft:item_display`, so it never
    /// occupies the cell or obstructs the hitbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_item: Option<String>,
    /// The offers, in declaration order — which is button order, and the order
    /// the `/trigger` routing values are assigned in.
    pub offers: Vec<ShopOffer>,
}

/// One button in a [`Shop`]: a gate, a label, and the effects choosing it runs.
///
/// **Effect root R8.** The bundle hangs off an object with runtime machinery of
/// its own (an advancement, a dialog, a trigger objective and a tick dispatch),
/// which is spec-0031's stated rule for adding a root rather than desugaring.
///
/// **Refusal is authored, not a field.** An offer whose own gate is closed is not
/// shown at all; an offer that should be *shown and refuse* leaves its own gate
/// open and gates its effects instead — the purchase behind `at-least <price>`,
/// the apology behind `at-most <price − 1>`. Both are the ordinary per-effect
/// gate every effect already carries, so the engine adds no `refused` field and
/// no second way to say one thing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShopOffer {
    /// The button's caption. Player-visible, inventoried under
    /// `shop.<id>.offer.<i>.label`, and width-checked like any dialog button.
    pub label: String,
    /// The button's hover tooltip — the natural home for a price in words.
    /// Player-visible, inventoried under `shop.<id>.offer.<i>.tooltip`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    /// Flags that must all be set for this offer to be shown (the one gate).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Flags whose being set hides this offer (the one gate).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Numeric gate terms (the one gate) — **this is where a price lives**.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_state: Vec<StateCompare>,
    /// What choosing this offer does. Effect root R8; runs with the choosing
    /// player as the acting player, so a `player`-scoped datum is writable here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<QuestEffect>,
}

impl ShopOffer {
    /// This offer's whole gate, as one value (DSL v0.10) — see
    /// [`crate::gate::Gate`].
    pub fn gate(&self) -> crate::gate::Gate<'_> {
        crate::gate::Gate::of(
            &self.requires_flags,
            &self.forbids_flags,
            &self.requires_state,
        )
    }
}

/// How much of a datum a death forfeits into a [`Stake`] (DSL v0.10, spec-0032).
///
/// A creator who wants **no death cost at all** picks [`Forfeit::None`]; the
/// stake then still marks where they fell, which is the memorial case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Forfeit {
    /// The whole balance.
    All,
    /// A percentage of the balance, rounded toward zero (integer arithmetic —
    /// ADR-0006 forbids anything a second implementation could round differently).
    Proportion {
        /// 0–100.
        percent: u32,
    },
    /// A fixed amount, capped at the balance so a purse can never go negative.
    Fixed {
        /// The amount.
        amount: i32,
    },
    /// Nothing is taken. The stake is a marker, not a wager.
    None,
}

/// What a death does when the player already holds the maximum number of live
/// stakes (DSL v0.10, spec-0032).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OnFull {
    /// Retire the player's oldest live stake — its contents are lost — and place a
    /// new one. With `max_live: 1` this is the souls behaviour.
    Replace,
    /// Leave the existing stakes alone: the new death places nothing and forfeits
    /// nothing. One wager at a time.
    Keep,
}

impl OnFull {
    /// The wire token (`replace` / `keep`).
    pub fn token(self) -> &'static str {
        match self {
            OnFull::Replace => "replace",
            OnFull::Keep => "keep",
        }
    }
}

/// Who may collect a stake that is not theirs (DSL v0.10, spec-0032).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CollectBy {
    /// Only the player who left it. The default.
    Owner,
    /// Anyone standing at it. The whole of every live stake at that place is
    /// transferred to the collector.
    Anyone,
}

impl CollectBy {
    /// The wire token (`owner` / `anyone`).
    pub fn token(self) -> &'static str {
        match self {
            CollectBy::Owner => "owner",
            CollectBy::Anyone => "anyone",
        }
    }
}

/// A stage-5 **recovery stake** (DSL v0.10, spec-0032): what a death leaves
/// behind, and the one chance to get it back.
///
/// # A mechanism, not a genre
///
/// "Souls" is one setting of this declaration and nothing here knows the word.
/// The mechanism is *a death forfeits some of a declared datum into a collectable
/// marker at a computed place, and collecting it returns exactly what was taken*.
/// A campaign with no death cost, a campaign with a permanent memorial at every
/// death site, and a campaign with the souls loop are three configurations, not
/// three engines.
///
/// # Where it lands is a compile-time answer
///
/// The owner's rule: *the anchor is the point, on the walkable path
/// from the respawn point in force at the moment of death to the death point
/// under the quest state in force at that moment, that minimises distance to the
/// death point.* Every term already has an owner in the compiler — walkability is
/// the navigation world the completability proof runs on, quest-state passability
/// is the DAG-indexed sealing `close-gate` established, and the respawn point in
/// force is engine state. So it is a table computed at build time and a lookup at
/// run time: **no runtime search, no nondeterminism.**
///
/// # It is hardware, so it inherits the hardware rules
///
/// The marker is an invisible `minecraft:interaction` for the hitbox plus a
/// glowing `minecraft:item_display` for the rendering — the same pair every other
/// affordance in the engine uses, and therefore the same invisible-affordance
/// (`DW0420`) and hardware-erasure (`DW0421`) proofs. It is deliberately **not**
/// a dropped item entity: an item despawns after 6000 ticks, burns in lava, sinks
/// in the void, and can be picked up by anyone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stake {
    /// Unique stake id (`stake/<kebab>`).
    pub id: StakeId,
    /// The datum a death forfeits into this stake, and a collection returns.
    /// Must be declared in the stage-5 `state` list, and must be `player`-scoped:
    /// a stake is a personal wager, and a party-shared purse would turn one
    /// player's death into everyone's penalty.
    pub state: StateId,
    /// How much of [`Self::state`] a death takes. Default: the whole balance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forfeit: Option<Forfeit>,
    /// How many live stakes one player may hold at once. Default 1; `0` means a
    /// death never places one (and never forfeits anything), which is the
    /// "no death cost" configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_live: Option<u32>,
    /// What a death does when the player is already at [`Self::max_live`].
    /// Default [`OnFull::Replace`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_full: Option<OnFull>,
    /// Who may collect. Default [`CollectBy::Owner`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collect_by: Option<CollectBy>,
    /// The line a collection says, on the action bar. Player-visible, inventoried
    /// under `stake.<id>.collected`.
    pub collected_message: String,
    /// The item the glowing marker renders as (default `minecraft:soul_lantern`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_item: Option<String>,
}

impl Stake {
    /// The forfeit rule, with the documented default applied.
    pub fn forfeit(&self) -> Forfeit {
        self.forfeit.unwrap_or(Forfeit::All)
    }

    /// How many live stakes one player may hold, with the documented default.
    pub fn max_live(&self) -> u32 {
        self.max_live.unwrap_or(1)
    }

    /// The at-capacity rule, with the documented default.
    pub fn on_full(&self) -> OnFull {
        self.on_full.unwrap_or(OnFull::Replace)
    }

    /// The collection rule, with the documented default.
    pub fn collect_by(&self) -> CollectBy {
        self.collect_by.unwrap_or(CollectBy::Owner)
    }

    /// The item the marker renders as, with the documented default.
    pub fn marker_item(&self) -> &str {
        self.marker_item
            .as_deref()
            .unwrap_or("minecraft:soul_lantern")
    }
}
