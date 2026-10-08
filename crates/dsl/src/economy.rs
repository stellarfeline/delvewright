//! Stage 5 — trade and the recovery stake (DSL v0.10, spec-0032).
//!
//! # A purchase that does not add up is refused where it is written (spec-0071 §2, `DW0901`)
//!
//! ## What this rule is for
//!
//! spec-0032 settled that a price is a gate term and that an offer's refusal is
//! authored: there is no `price` field, because *"may this happen yet?"* already
//! has an owner ([`crate::gate::Gate`]). That ruling stands, and this rule is its
//! missing half.
//!
//! Its cost is that one number stands in four or five places with nothing binding
//! the copies — in the button's own words, on the `at-least` gate of what the
//! player receives, on the `at-least` gate of the charge, as the charge's
//! `amount`, and, minus one, on the `at-most` gate of the line that apologises.
//! An author moves one of them and the rest stay where they are. This is what
//! compares them: an offer gating on `at-least 15` and charging `16` drives a
//! purse one below the floor its own gate states, and that is a refusal rather
//! than a shipped delve.
//!
//! ## The rule, and what it binds to
//!
//! Every **effect list whose effects charge a state** — not every shop, and not
//! every offer. A charge is an `add-state` moving a datum by a negative amount;
//! the rule reads the gate, so it binds wherever the shape occurs: a shop offer,
//! a trigger, a trap payload, a quest's `on_complete`, a `sequence`'s step. A
//! shop is the shape's commonest home, never what the rule is about.
//!
//! For one list `L` and one datum `S` that some effect of `L` charges:
//!
//! 1. **The charge is covered.** An effect charging `−n` may only fire at a
//!    balance that can pay it: the floor its own gate and its list's enclosing
//!    gate leave open must be at least `n`. A charge nothing floors is refused
//!    too, **where the list prices the same datum somewhere else** — an ungated
//!    debit beside a gated purchase is the same defect with one copy of the
//!    number missing rather than wrong. Where nothing else in the list mentions
//!    the datum there is no second copy to disagree with, and a campaign that
//!    means to drive a datum below zero is a design, so the rule says nothing.
//! 2. **The sell and the refusal meet exactly.** An effect gated `S at-most k`
//!    and nothing else on `S` is the arm that plays when the player cannot pay.
//!    Its ceiling must be one below the lowest balance at which the charge fires:
//!    `k = m − 1`. A lower `k` leaves a **gap** — balances at which the list
//!    neither sells nor refuses, and the button does nothing at all. A `k` at or
//!    above `m` leaves an **overlap** — balances at which the player is both
//!    charged and told they cannot afford it.
//!
//! An effect that declares both a floor and a ceiling on `S` states an interval
//! and is not a refusal arm, so rule 2 leaves it alone: two bounds on one datum
//! are a range the creator wrote on purpose, not two copies of one price.
//!
//! ## The enclosing gate
//!
//! A list hangs off something, and that something may gate it: a shop offer's own
//! gate is the canonical case (*the button is not shown below the price*), and a
//! trigger's and a trap's arming gates are the same fact one level out. A nested
//! list's enclosing gate is the parent effect's `when`. The floor is read from
//! the conjunction, because both must hold for the charge to run — which is what
//! lets the correct spelling of a shop (gate the offer, leave the effects bare)
//! pass while the incorrect one (gate the offer at 15, charge 16) is refused.
//!
//! ## Binding
//!
//! [`PurchaseBinding`] states what the rule examined on this campaign: lists
//! walked, charges counted, `(list, datum)` pairs bound, and refusals — with the
//! denominator, because a rule that binds to nothing is vacuous rather than green
//! (CLAUDE.md).

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

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::gate::DatumSet;
use crate::registry::AnchorRegistry;
use crate::validate::{AnchorProviders, station_kind_diag};
use crate::{CompareOp, StateWrite};

/// A shop's anchor must be provided by some prefab bound in this campaign
/// (DSL v0.10, spec-0032) — the same rule, and the same message shape, every
/// other stage-5 anchor reference follows.
pub(crate) fn shop_anchor_checks(
    c: &Campaign,
    anchors: &dyn AnchorRegistry,
    d: &mut Vec<Diagnostic>,
) {
    if c.quests.content.shops.is_empty() {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);
    for (i, sh) in c.quests.content.shops.iter().enumerate() {
        if let Some(f) = station_kind_diag(
            &providers,
            sh.anchor.as_str(),
            crate::layout::StationKind::Point,
            "a shop counter",
            "quests",
            format!("/content/shops/{i}/anchor"),
        ) {
            d.push(f);
        }
        if providers.resolvable(sh.anchor.as_str()) {
            continue;
        }
        d.push(Diagnostic::error(
            codes::ANCHOR_UNRESOLVED,
            "quests",
            format!("/content/shops/{i}/anchor"),
            format!(
                "shop anchor `{}` is not provided by any prefab bound in this campaign — {}",
                sh.anchor,
                providers.anchor_remedy(
                    "use an anchor the prefab exposes (anchor names come from prefab metadata; do \
                     NOT invent one)"
                ),
            ),
        ));
    }
}

/// Stage-5 economy checks (DSL v0.10, spec-0032): the shop's structural
/// obligations and the stake's declaration obligations.
///
/// **What is deliberately NOT here.** Everything about *where* a stake lands —
/// the placement table, its walkability, its reachability from the respawn point
/// in force, and whether the block under it is one runtime removes — is a
/// question about the solved layout and the navigation world, which the DSL crate
/// does not have. Those live in `crate::stake` on the compiler side, exactly as a
/// lethal volume's geometry does.
///
/// **No price check appears here either**, and that is the design showing
/// through: a price is a [`crate::gate::Gate`] term, so it is already covered by
/// `DW0500`–`DW0503` — the datum must be declared, must be written somewhere,
/// must be read somewhere, and must be reachable at the scope the site evaluates
/// at. A shop that added a comparison field of its own would have needed all four
/// rules written a second time.
pub(crate) fn economy_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let stakes = &c.quests.content.stakes;
    let shops = &c.quests.content.shops;
    if stakes.is_empty() && shops.is_empty() {
        return;
    }

    // --- stakes: id hygiene, the datum, the scope, the forfeit range ----------
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, s) in stakes.iter().enumerate() {
        if !s.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/stakes/{i}/id"),
                format!(
                    "malformed stake id `{}` — stake ids are `stake/<kebab-case>`",
                    s.id
                ),
            ));
        }
        if !seen.insert(s.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/stakes/{i}/id"),
                format!("duplicate stake id `{}`", s.id),
            ));
        }
        match c.quests.content.state_decl(s.state.as_str()) {
            None => d.push(Diagnostic::error(
                codes::STAKE_STATE_SCOPE,
                "quests",
                format!("/content/stakes/{i}/state"),
                format!(
                    "stake `{}` forfeits `{}`, which the campaign never declares. Add it to the \
                     stage-5 `state` list with `\"scope\": \"player\"` — a stake is one player's \
                     wager, and a datum's scope is a fact no use site can supply",
                    s.id,
                    s.state.as_str()
                ),
            )),
            Some(decl) if decl.scope != crate::StateScope::Player => {
                d.push(Diagnostic::error(
                    codes::STAKE_STATE_SCOPE,
                    "quests",
                    format!("/content/stakes/{i}/state"),
                    format!(
                        "stake `{}` forfeits `{}`, which is declared `party`-scoped. A stake is a \
                         PERSONAL wager: one shared purse turns a teammate's death into a penalty \
                         on everyone, and nothing in the JSON would say so. Declare the datum \
                         `player`-scoped, or point the stake at one that is.",
                        s.id,
                        s.state.as_str()
                    ),
                ));
            }
            Some(_) => {}
        }
        if let Some(crate::Forfeit::Proportion { percent }) = s.forfeit
            && percent > 100
        {
            d.push(Diagnostic::error(
                codes::STAKE_FORFEIT_RANGE,
                "quests",
                format!("/content/stakes/{i}/forfeit/percent"),
                format!(
                    "stake `{}` forfeits {percent}% of `{}` — more than the whole purse. Use \
                     0–100, or `{{\"kind\": \"all\"}}`.",
                    s.id,
                    s.state.as_str()
                ),
            ));
        }
    }

    // --- `drop-stake`: every reference resolves, every declaration is dropped --
    // Both halves, for the reason `DW0501`/`DW0502` state for a datum: a
    // reference with no declaration is a runtime no-op, and a declaration no beat
    // fires is a whole mechanism that binds to nothing.
    let mut dropped: BTreeSet<String> = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |path, _site, eff| {
        let crate::Verb::DropStake { stake, .. } = &eff.verb else {
            return;
        };
        if c.quests.content.stake_decl(stake.as_str()).is_none() {
            d.push(Diagnostic::error(
                codes::STAKE_UNDECLARED,
                "quests",
                path.to_string(),
                format!(
                    "`drop-stake` leaves `{}`, which the campaign never declares. Add it to the \
                     stage-5 `stakes` list, or fix the id",
                    stake.as_str()
                ),
            ));
            return;
        }
        dropped.insert(stake.as_str().to_string());
    });
    for (i, s) in stakes.iter().enumerate() {
        if dropped.contains(s.id.as_str()) {
            continue;
        }
        d.push(Diagnostic::error(
            codes::STAKE_NEVER_DROPPED,
            "quests",
            format!("/content/stakes/{i}/id"),
            format!(
                "stake `{}` is declared and no `drop-stake` effect anywhere in the campaign ever \
                 leaves one. Its forfeit rule, its retention policy and its whole compile-time \
                 placement table describe a mechanism no beat can fire. Drop it from a beat — \
                 `on_death` is the usual one — or delete the declaration.",
                s.id
            ),
        ));
    }

    crate::state::read_after_write_checks(c, d);

    // --- shops: id hygiene, and no button that cannot answer ------------------
    let mut seen_shop: BTreeSet<&str> = BTreeSet::new();
    for (i, sh) in shops.iter().enumerate() {
        if !sh.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/shops/{i}/id"),
                format!(
                    "malformed shop id `{}` — shop ids are `shop/<kebab-case>`",
                    sh.id
                ),
            ));
        }
        if !seen_shop.insert(sh.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/shops/{i}/id"),
                format!("duplicate shop id `{}`", sh.id),
            ));
        }
        if sh.offers.is_empty() {
            d.push(Diagnostic::error(
                codes::SHOP_OFFER_INERT,
                "quests",
                format!("/content/shops/{i}/offers"),
                format!(
                    "shop `{}` declares no offers. Vanilla's dialog codec rejects an empty action \
                     list outright, so this is not merely an empty shop — it is a dialog that \
                     fails to load. Give it at least one offer, or delete the shop.",
                    sh.id
                ),
            ));
        }
        for (j, off) in sh.offers.iter().enumerate() {
            if off.effects.is_empty() {
                d.push(Diagnostic::error(
                    codes::SHOP_OFFER_INERT,
                    "quests",
                    format!("/content/shops/{i}/offers/{j}/effects"),
                    format!(
                        "offer `{}` in shop `{}` declares no effects: the button is drawn, is \
                         pressable, and does nothing. A control the player can operate must have \
                         an answer — a refusal counts, so a single `narrate` gated on \
                         `requires_state` (`at-most <price − 1>`) is enough.",
                        off.label, sh.id
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Purchases (spec-0071 §2)
// ---------------------------------------------------------------------------

/// One effect list the rule judges, with the gate the thing it hangs off imposes.
struct EffectList<'a> {
    /// JSON pointer to the list.
    path: String,
    /// The stage document the list lives in.
    stage: &'static str,
    /// Numeric terms every effect in the list is additionally subject to — the
    /// owner's own gate (a shop offer, a trigger, a trap) or the parent effect's
    /// `when` for a nested list.
    context: Vec<StateCompare>,
    /// Where the enclosing gate is written, for the message.
    context_path: Option<String>,
    /// The effects, in declaration order.
    effects: &'a [QuestEffect],
}

/// Every effect list in the campaign, each with its enclosing gate, in one fixed
/// order: the roots from the single effect-root enumeration, and every list
/// nested inside an effect through the single nesting authority.
fn effect_lists(c: &Campaign) -> Vec<EffectList<'_>> {
    let mut out: Vec<EffectList<'_>> = Vec::new();
    crate::effects::for_each_effect_root(c, &mut |site, list| {
        // What gates the whole bundle, where the owner declares one. A quest
        // bundle, a dialogue `on_respawn`, a shortcut's `on_unlock` and the
        // campaign's `on_death` carry no gate of their own, so their lists are
        // judged on the effects' own guards alone.
        let (context, context_path): (Vec<StateCompare>, Option<String>) = match site.owner {
            crate::effects::EffectRootOwner::Trigger(t) => (
                t.gate().requires_state.to_vec(),
                Some(trim_suffix(&site.path, "/effects")),
            ),
            crate::effects::EffectRootOwner::TrapPayload(t) => (
                t.gate().requires_state.to_vec(),
                Some(trim_suffix(&site.path, "/payload")),
            ),
            crate::effects::EffectRootOwner::ShopOffer(shop) => {
                let offer = offer_index(&site.path).and_then(|i| shop.offers.get(i));
                (
                    offer
                        .map(|o| o.gate_view().requires_state.to_vec())
                        .unwrap_or_default(),
                    Some(trim_suffix(&site.path, "/effects")),
                )
            }
            _ => (Vec::new(), None),
        };
        out.push(EffectList {
            path: site.path.clone(),
            stage: site.stage,
            context,
            context_path,
            effects: list,
        });
        for (i, eff) in list.iter().enumerate() {
            nested_lists(eff, &format!("{}/{i}", site.path), site.stage, &mut out);
        }
    });
    out
}

/// Every list nested inside `eff`, transitively, each gated by its parent's own
/// `when` — the numeric terms that must hold for the parent to run at all.
fn nested_lists<'a>(
    eff: &'a QuestEffect,
    path: &str,
    stage: &'static str,
    out: &mut Vec<EffectList<'a>>,
) {
    for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
        let lpath = format!("{path}/{pseg}");
        out.push(EffectList {
            path: lpath.clone(),
            stage,
            context: eff.requires_state().to_vec(),
            context_path: Some(format!("{path}/when")),
            effects: list,
        });
        for (i, inner) in list.iter().enumerate() {
            nested_lists(inner, &format!("{lpath}/{i}"), stage, out);
        }
    }
}

/// `/content/shops/3/offers/2/effects` → `2` (segment 5; segment 4 is the
/// literal `offers`).
fn offer_index(path: &str) -> Option<usize> {
    path.split('/').nth(5).and_then(|n| n.parse().ok())
}

/// `path` without a trailing `suffix` (the list's own field name), i.e. the
/// object that declares the enclosing gate.
fn trim_suffix(path: &str, suffix: &str) -> String {
    path.strip_suffix(suffix).unwrap_or(path).to_string()
}

/// The lowest balance of `state` at which every one of `terms` holds, or `None`
/// when nothing bounds it from below (or when no balance satisfies them at all —
/// an unsatisfiable gate is `DW0344`'s finding, not this rule's).
fn floor_of(terms: &[&StateCompare], state: &str) -> Option<i32> {
    let mut set = DatumSet::all();
    let mut bounded = false;
    for t in terms {
        if t.state.as_str() != state {
            continue;
        }
        if matches!(t.op, CompareOp::AtLeast | CompareOp::Equals) {
            bounded = true;
        }
        set.require(t.op, t.value);
    }
    bounded.then(|| set.min()).flatten()
}

/// The highest balance of `state` at which every one of `terms` holds, or `None`
/// when nothing bounds it from above.
fn ceiling_of(terms: &[&StateCompare], state: &str) -> Option<i32> {
    let mut set = DatumSet::all();
    let mut bounded = false;
    for t in terms {
        if t.state.as_str() != state {
            continue;
        }
        if matches!(t.op, CompareOp::AtMost | CompareOp::Equals) {
            bounded = true;
        }
        set.require(t.op, t.value);
    }
    bounded.then(|| set.max()).flatten()
}

/// The terms in scope for one effect of a list: the list's enclosing gate and the
/// effect's own `when`, which is the conjunction the effect actually fires under.
fn in_scope<'a>(list: &'a EffectList<'_>, eff: &'a QuestEffect) -> Vec<&'a StateCompare> {
    list.context
        .iter()
        .chain(eff.requires_state().iter())
        .collect()
}

/// `DW0901` over every effect list that charges a datum (spec-0071 §2).
pub fn purchase_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for list in effect_lists(c) {
        for state in charged_states(&list) {
            judge(&list, &state, d);
        }
    }
}

/// The datums some effect of this list charges — the rule's bound objects, in a
/// deterministic order.
fn charged_states(list: &EffectList<'_>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for eff in list.effects {
        if let Some((state, StateWrite::Add(n))) = eff.writes_state()
            && n < 0
        {
            out.insert(state.as_str().to_string());
        }
    }
    out
}

/// One `(list, datum)` pair: the two arms of §2, in the order an author reads
/// them — first whether each charge is covered, then whether the refusal arm
/// meets the sale.
fn judge(list: &EffectList<'_>, state: &str, d: &mut Vec<Diagnostic>) {
    // The lowest balance at which any charge of this list fires — the boundary a
    // refusal arm must stop one below.
    let mut sell_floor: Option<i32> = None;
    for (i, eff) in list.effects.iter().enumerate() {
        let Some((s, StateWrite::Add(amount))) = eff.writes_state() else {
            continue;
        };
        if s.as_str() != state || amount >= 0 {
            continue;
        }
        let charge = -(amount as i64);
        let terms = in_scope(list, eff);
        let floor = floor_of(&terms, state);
        match floor {
            Some(f) if (f as i64) >= charge => {}
            Some(f) => d.push(Diagnostic::error(
                codes::PURCHASE_ARITHMETIC,
                list.stage,
                format!("{}/{i}", list.path),
                format!(
                    "this effect charges {charge} of `{state}` behind a gate that opens at \
                     {f}: at a balance of {f} the charge leaves `{state}` at {}, below the \
                     floor the gate states. The two numbers are one price written twice — \
                     raise the gate to `at-least {charge}`, or charge {f}{}",
                    (f as i64) - charge,
                    gate_note(list),
                ),
            )),
            // Nothing floors the charge. That is only a defect where the list
            // is otherwise written as a purchase — some other effect, or the
            // enclosing gate, states a term on the same datum. A list that says
            // nothing else about `S` has no second copy of the number to
            // disagree with, and a datum a campaign means to drive below zero is
            // a design this rule has no standing to refuse.
            None if priced_elsewhere(list, state, i) => d.push(Diagnostic::error(
                codes::PURCHASE_ARITHMETIC,
                list.stage,
                format!("{}/{i}", list.path),
                format!(
                    "this effect charges {charge} of `{state}` and nothing gates it on what \
                     `{state}` holds, in a list that prices the same datum elsewhere. It fires \
                     at any balance, including one that cannot pay — gate it `at-least \
                     {charge}` in its own `when`{}",
                    gate_note(list),
                ),
            )),
            None => {}
        }
        if let Some(f) = floor {
            sell_floor = Some(sell_floor.map_or(f, |m: i32| m.min(f)));
        }
    }
    let Some(m) = sell_floor else { return };
    for (i, eff) in list.effects.iter().enumerate() {
        let terms = in_scope(list, eff);
        // A refusal arm: an upper bound on the datum and no lower one. An effect
        // stating both wrote an interval, which is a range and not a price.
        if floor_of(&terms, state).is_some() {
            continue;
        }
        let Some(k) = ceiling_of(&terms, state) else {
            continue;
        };
        if k == m - 1 {
            continue;
        }
        let (what, balances) = if k < m - 1 {
            (
                "leaves a gap",
                format!(
                    "at a balance of {}..{} this list neither charges nor answers",
                    k + 1,
                    m - 1
                ),
            )
        } else {
            (
                "overlaps the sale",
                format!(
                    "at a balance of {}..{k} this list both charges and answers as if it could \
                     not",
                    m
                ),
            )
        };
        d.push(Diagnostic::error(
            codes::PURCHASE_ARITHMETIC,
            list.stage,
            format!("{}/{i}", list.path),
            format!(
                "this effect answers below `{state} at-most {k}` while the charge in the same \
                 list fires from {m}, which {what}: {balances}. The two literals are one price \
                 — the answering arm's ceiling is `at-most {}`",
                m - 1
            ),
        ));
    }
}

/// Whether anything in this list but effect `skip` states a term on `state` —
/// the enclosing gate, or another effect's own `when`. What makes a list "a
/// purchase" rather than a place a datum happens to move.
fn priced_elsewhere(list: &EffectList<'_>, state: &str, skip: usize) -> bool {
    list.context.iter().any(|t| t.state.as_str() == state)
        || list
            .effects
            .iter()
            .enumerate()
            .any(|(j, e)| j != skip && e.requires_state().iter().any(|t| t.state.as_str() == state))
}

/// The clause naming the enclosing gate, when the list has one — a creator whose
/// price is on the offer must be told which of the two numbers the rule read.
fn gate_note(list: &EffectList<'_>) -> String {
    match &list.context_path {
        Some(p) if !list.context.is_empty() => {
            format!(" (the gate at `{p}` is read together with this effect's own `when`)")
        }
        _ => String::new(),
    }
}

/// What the purchase rule examined, zeroes included (spec-0071 §2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PurchaseBinding {
    /// Effect lists walked — the denominator.
    pub lists: usize,
    /// Lists with an enclosing gate that states a numeric term.
    pub gated_lists: usize,
    /// Charges the walk counts (an `add-state` moving a datum down).
    pub charges: usize,
    /// `(list, datum)` pairs bound — the objects the rule judges.
    pub pairs: usize,
    /// Distinct datums charged anywhere in the campaign.
    pub datums: usize,
    /// Diagnostics raised (`DW0901`).
    pub refused: usize,
}

impl PurchaseBinding {
    /// Count what [`purchase_checks`] examines on `c`.
    pub fn of(c: &Campaign) -> Self {
        let mut b = PurchaseBinding::default();
        let mut datums: BTreeSet<String> = BTreeSet::new();
        let mut per_list: BTreeMap<String, usize> = BTreeMap::new();
        for list in effect_lists(c) {
            b.lists += 1;
            if !list.context.is_empty() {
                b.gated_lists += 1;
            }
            for eff in list.effects {
                if let Some((state, StateWrite::Add(n))) = eff.writes_state()
                    && n < 0
                {
                    b.charges += 1;
                    datums.insert(state.as_str().to_string());
                    *per_list
                        .entry(format!("{}|{}", list.path, state.as_str()))
                        .or_default() += 1;
                }
            }
        }
        b.pairs = per_list.len();
        b.datums = datums.len();
        let mut d = Vec::new();
        purchase_checks(c, &mut d);
        b.refused = d.len();
        b
    }

    /// The one line this rule owes its reader.
    pub fn line(&self) -> String {
        format!(
            "purchase binding: {} charge(s) over {} datum(s) bind {} (list, datum) pair(s) of {} \
             effect list(s) walked, {} of them behind a numeric gate, {} refused (DW0901).",
            self.charges, self.datums, self.pairs, self.lists, self.gated_lists, self.refused
        )
    }
}
