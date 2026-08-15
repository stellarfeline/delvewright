//! Effect-timeline gate state — the static half of the `close-gate` model
//! (`DW0410`).
//!
//! `crate::nav`'s existing seal model reasons about the **quest DAG**: which
//! gates are shut while the *player* walks a critical-path leg, rooted in
//! objective causality ([`crate::plan::Plan::gate_fired_before`]). That model
//! deliberately says nothing about the order of two effects inside one bundle,
//! because across bundles there is no order to know.
//!
//! Inside a **single effect timeline** there is. Round-8 island playtest: one
//! `sequence` closed the boulder gate at `at_ticks: 460` and walked an actor at
//! `at_ticks: 700` along a path straight through the region the boulder now
//! filled. The compiler proved that walk on the *open* world — gates are
//! modelled passable — and at runtime the giant stepped through solid basalt.
//! The gate state at tick 700 was never in doubt; nothing was looking.
//!
//! This module replays each timeline and records, for every effect, the gate
//! regions an **earlier effect in the same timeline** provably sealed. A
//! timeline is:
//!
//! * one **effect root** — every list [`crate::plan::for_each_effect_root`]
//!   enumerates, i.e. the five emission can lower: a quest's
//!   `on_objective_complete[obj]` bundle, its `on_complete`, a trigger's
//!   `effects`, a `traps[].payload`, and a dialogue option's `set-checkpoint`
//!   `on_respawn` bundle. Effects run in declared order, in one tick, so effect
//!   *j* has finished before effect *i > j* begins;
//! * a `sequence`, whose steps are ordered by `(at_ticks, declaration index)` —
//!   real elapsed time, which is exactly what the island defect turned on;
//! * a `move-actor` / `move-npc` `on_arrive` bundle, which inherits the state as
//!   of its move and is itself an ordered list.
//!
//! ## Optional roots need no special case
//!
//! Two of the five have no guaranteed firing: the party may never trip a trap,
//! and nobody is forced to die at a checkpoint. The completability model has to
//! rule on that (an unguaranteed firing registers its `close-gate`
//! only, because assuming a seal happened is the conservative direction) —
//! **this model does not**, and the asymmetry is not an oversight.
//!
//! `collect_region_events` reasons *across* bundles about the route the player is
//! forced to walk, so whether a firing happens is load-bearing. The staged walk
//! reasons only *within* one bundle, about a walk that bundle itself orders, and
//! its claim is conditional from the start: **if this bundle runs, this walk
//! starts after that seal landed**. A trap that never springs never contradicts
//! it, because it never runs the walk either. Optionality therefore cancels on
//! both sides of the implication, and the two new roots are ordinary timelines —
//! no `EffectRoot` arm, no weakening. A payload's walk must be legal in the world
//! its own payload has already made, which is exactly what it must be whenever it
//! fires.
//!
//! ## No false certainty
//!
//! The stance is [`crate::continuity`]'s: state only what the structure proves.
//!
//! * **Cross-bundle order is unknowable** and is never guessed — every timeline
//!   starts from "no gate provably sealed", regardless of what other quests,
//!   triggers or dialogue options do. A campaign whose seal happens in another
//!   bundle simply gets no diagnostic here; the DAG-causal model
//!   (`DW0311`/`DW0315`) is what covers the player's forced route.
//! * **A conditional firing proves nothing.** A `close-gate` carrying
//!   `requires_flags`/`forbids_flags` may not fire, so it never adds a seal; a
//!   conditional `open-gate` may not fire either, so it *drops* the region back
//!   to unsealed rather than asserting it is open. Both directions collapse to
//!   "not provably sealed" — the direction that can only ever withhold an error,
//!   never invent one.
//! * **Nested state does not leak outward.** Gate effects inside an `on_arrive`
//!   fire when the walk lands, which is not ordered against the enclosing
//!   bundle's later siblings, so they seal only within that bundle.
//!
//! Symmetrically, a path may **rely** on a gate an earlier effect opened: the
//! occupancy model already treats every gate region as passable, so an
//! `open-gate` needs no special case — it just clears any seal this timeline had
//! established.

use std::collections::BTreeMap;

use delvewright_dsl::{Campaign, QuestEffect};

use crate::plan::{Plan, ResolvedAnchor};

/// A gate region: the inclusive world-space corners the gate anchor spans.
pub type Region = ([i32; 3], [i32; 3]);

/// The gate regions a timeline has provably sealed at some point in its replay,
/// each mapped to the gate anchor name whose `close-gate` sealed it (so a
/// diagnostic can name the culprit rather than a pair of coordinates).
///
/// A `BTreeMap` keyed by region: deterministic iteration, and one entry per
/// region so a later `open-gate`/`close-gate` on the same region simply
/// overwrites the earlier verdict.
pub type GateState = BTreeMap<Region, String>;

/// Replay every timeline in the campaign, yielding each effect paired with the
/// gate state **as of that effect** — the regions an earlier effect in its own
/// timeline provably sealed.
///
/// The traversal order is the compiler's canonical effect pre-order:
/// [`crate::plan::for_each_effect_root`]'s five roots in their fixed order, each
/// list in declaration order, each effect yielded before its own nested effects.
/// [`crate::nav::all_effects`] is defined as this walk with the states dropped,
/// so the two can never drift apart — the alignment is structural, not a
/// convention two functions have to remember. Sharing the root enumeration
/// upward buys the same guarantee against the gate scans and the emitter.
pub fn walk<'a>(plan: &'a Plan) -> Vec<(&'a QuestEffect, GateState)> {
    walk_campaign(plan.campaign, &plan.anchors)
}

/// [`walk`] against a bare campaign + resolved anchor table (the unit-testable
/// core; `Plan` is only ever used for those two fields).
pub fn walk_campaign<'a>(
    c: &'a Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<(&'a QuestEffect, GateState)> {
    let mut out = Vec::new();
    // Every root is its own timeline, in the one order
    // [`crate::plan::for_each_effect_root`] fixes. They are concatenated to match
    // the canonical pre-order, but no seal crosses from one into the next — which
    // is why the enumeration needs no per-root reasoning here at all.
    crate::plan::for_each_effect_root(c, &mut |_site, effs| {
        walk_list(effs, &GateState::new(), anchors, &mut out);
    });
    out
}

/// Replay one ordered effect list, starting from `state_in`. Each effect is
/// pushed with the state that holds when it fires, then its own gate verb is
/// applied so it lands on the *following* siblings.
fn walk_list<'a>(
    effs: &'a [QuestEffect],
    state_in: &GateState,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    out: &mut Vec<(&'a QuestEffect, GateState)>,
) {
    let mut state = state_in.clone();
    for e in effs {
        out.push((e, state.clone()));
        walk_children(e, &state, anchors, out);
        apply(e, &mut state, anchors);
    }
}

/// Descend into an effect's nested timelines, in the canonical (declaration)
/// order the pre-order requires.
fn walk_children<'a>(
    e: &'a QuestEffect,
    state: &GateState,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    out: &mut Vec<(&'a QuestEffect, GateState)>,
) {
    match e {
        QuestEffect::Sequence { steps } => {
            // A sequence's steps fire at real tick offsets, so their causal order
            // is `(at_ticks, declaration index)` — NOT declaration order. This is
            // the ordering the island defect turned on: the `close-gate` sits at
            // `at_ticks: 460` and the walk at `at_ticks: 700`, and only the tick
            // offsets say which came first. Ties break on declaration index,
            // matching the emitter (same-tick steps run in declared order).
            let mut order: Vec<usize> = (0..steps.len()).collect();
            order.sort_by_key(|&i| (steps[i].at_ticks, i));
            let mut prefix: Vec<GateState> = vec![GateState::new(); steps.len()];
            let mut acc = state.clone();
            for &i in &order {
                prefix[i] = acc.clone();
                for inner in &steps[i].effects {
                    apply(inner, &mut acc, anchors);
                }
            }
            // Emitted in DECLARATION order (the canonical pre-order), each with
            // the state its tick offset earned it.
            for (i, step) in steps.iter().enumerate() {
                walk_list(&step.effects, &prefix[i], anchors, out);
            }
        }
        QuestEffect::MoveActor { on_arrive, .. } | QuestEffect::MoveNpc { on_arrive, .. } => {
            // Fires when the walk lands: inherits the state at the move, and its
            // own gate effects stay inside (they are not ordered against the
            // enclosing bundle's later siblings).
            walk_list(on_arrive, state, anchors, out);
        }
        _ => {}
    }
}

/// Fold one effect's gate verb into the timeline state.
///
/// Unconditional `close-gate` ⇒ the region is provably sealed from here on.
/// Anything else that touches the region — an `open-gate`, or a *conditional*
/// gate verb in either direction — drops it back to "not provably sealed". See
/// the module's no-false-certainty note: the model only ever asserts seals it
/// can prove, so every uncertainty resolves toward silence.
fn apply(
    e: &QuestEffect,
    state: &mut GateState,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) {
    let conditional = !e.requires_flags().is_empty() || !e.forbids_flags().is_empty();
    if let Some(a) = e.close_gate_anchor() {
        if let Some(region) = gate_region(anchors, a.as_str()) {
            if conditional {
                state.remove(&region);
            } else {
                state.insert(region, a.as_str().to_string());
            }
        }
    } else if let Some(a) = e.open_gate_anchor()
        && let Some(region) = gate_region(anchors, a.as_str())
    {
        state.remove(&region);
    }
}

/// Resolve a gate anchor name to its region, scanning every area (first match) —
/// gate effects carry no area, exactly as `crate::plan`'s collector resolves them.
/// A name that is not a gate anchor yields `None` and contributes no state (a
/// validation concern: `DW0142`/`DW0343`).
fn gate_region(anchors: &BTreeMap<(String, String), ResolvedAnchor>, name: &str) -> Option<Region> {
    anchors
        .iter()
        .find_map(|((_, n), resolved)| match resolved {
            ResolvedAnchor::Gate { from, to, .. } if n == name => Some((*from, *to)),
            _ => None,
        })
}
