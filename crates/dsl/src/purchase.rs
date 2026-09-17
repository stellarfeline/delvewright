//! **A purchase that does not add up is refused where it is written**
//! (spec-0071 §2, `DW0901`).
//!
//! # What this rule is for
//!
//! spec-0032 settled that a price is a gate term and that an offer's refusal is
//! authored: there is no `price` field, because *"may this happen yet?"* already
//! has an owner ([`crate::gate::Gate`]). That ruling stands, and this rule is its
//! missing half.
//!
//! Its cost is that one number is written four or five times with nothing binding
//! the copies — once in the button's own words, once as the `at-least` gate on
//! what the player receives, once as the `at-least` gate on the charge, once as
//! the charge's `amount`, and once (minus one) as the `at-most` gate on the line
//! that apologises. Nothing compared them, so an offer that gates on `at-least
//! 15` and charges `16` compiled, shipped, and drove a purse one below the floor
//! the creator had gated on. Measured on a real quest stage: twenty constructor
//! functions in a 960-line script, and the first of them existed to stop that
//! number being typed five times.
//!
//! # The rule, and what it binds to
//!
//! Every **effect list whose effects charge a state** — not every shop, and not
//! every offer. A charge is an `add-state` moving a datum by a negative amount;
//! the rule reads the gate, so it binds wherever the shape occurs: a shop offer,
//! a trigger, a trap payload, a quest's `on_complete`, a `sequence`'s step. Shops
//! are where it was found, never what it is about.
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
//! # The enclosing gate
//!
//! A list hangs off something, and that something may gate it: a shop offer's own
//! gate is the canonical case (*the button is not shown below the price*), and a
//! trigger's and a trap's arming gates are the same fact one level out. A nested
//! list's enclosing gate is the parent effect's `when`. The floor is read from
//! the conjunction, because both must hold for the charge to run — which is what
//! lets the correct spelling of a shop (gate the offer, leave the effects bare)
//! pass while the incorrect one (gate the offer at 15, charge 16) is refused.
//!
//! # Binding
//!
//! [`PurchaseBinding`] states what the rule examined on this campaign: lists
//! walked, charges found, `(list, datum)` pairs bound, and refusals — with the
//! denominator, because a rule that binds to nothing is vacuous rather than green
//! (CLAUDE.md).

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::gate::DatumSet;
use crate::stages::{CompareOp, QuestEffect, StateCompare, StateWrite};

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
    /// Charges found (an `add-state` moving a datum down).
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
