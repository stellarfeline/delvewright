//! **The two classes a fight takes** — a wave (`dw_wave_<id>`) and an actor
//! (`dw_actor_<id>`) — and the static facts about them that more than one rule
//! reads.
//!
//! A property that acts on *a body being killed* belongs to both classes, the
//! pair that already carries `tier` and `equipment` as one shape. [`Fight`] is
//! that pair as one value, so a consumer that reasons about "a fight" matches on
//! it once rather than writing a wave arm and an actor arm that can drift.
//!
//! The facts here are read by the document-tier rules (`DW0913`) and by the
//! compiler (`plan::wave_area`, `combat::hostile_actors`), and each is stated
//! once:
//!
//! * [`wave_area`] — the area a wave's bodies are seated in, resolved from the
//!   beat that spawns it. `None` is the fact that leaves a wave without any
//!   runtime machinery.
//! * [`unleashed_actors`] — every actor some `unleash-actor` names, at any effect
//!   root and any nesting: the definition of an actor that is a fight.
//!
//! Determinism (ADR-0006): every walk is over slices and `BTreeSet`s.

use std::collections::BTreeSet;

use crate::envelope::Campaign;
use crate::stages::{Actor, Objective, OnKill, QuestEffect, Verb, Wave, for_each_campaign_effect};

/// One fight the campaign declares: a wave or an actor.
#[derive(Clone, Copy, Debug)]
pub enum Fight<'a> {
    /// A `waves[]` entry — its bodies wear `dw_wave_<id>`.
    Wave(&'a Wave),
    /// An `actors[]` entry — its body wears `dw_actor_<id>`.
    Actor(&'a Actor),
}

impl<'a> Fight<'a> {
    /// The kebab word a reader of a diagnostic knows the class by.
    pub fn word(&self) -> &'static str {
        match self {
            Fight::Wave(_) => "wave",
            Fight::Actor(_) => "actor",
        }
    }

    /// The declared id (`wave/<kebab>` or `actor/<kebab>`).
    pub fn id(&self) -> &'a str {
        match self {
            Fight::Wave(w) => w.id.as_str(),
            Fight::Actor(a) => a.id.as_str(),
        }
    }

    /// The fight's `on_kill` bundle, if it declares one (spec-0074).
    pub fn on_kill(&self) -> Option<&'a OnKill> {
        match self {
            Fight::Wave(w) => w.on_kill.as_ref(),
            Fight::Actor(a) => a.on_kill.as_ref(),
        }
    }
}

/// Every fight the campaign declares, waves first then actors, each in
/// declaration order, with the JSON pointer to its declaration in the quests
/// document.
pub fn fights(c: &Campaign) -> Vec<(String, Fight<'_>)> {
    let content = &c.quests.content;
    let mut out = Vec::new();
    for (i, w) in content.waves.iter().enumerate() {
        out.push((format!("/content/waves/{i}"), Fight::Wave(w)));
    }
    for (i, a) in content.actors.iter().enumerate() {
        out.push((format!("/content/actors/{i}"), Fight::Actor(a)));
    }
    out
}

/// Every actor some `unleash-actor` names, at any effect root and any nesting —
/// the one definition of an actor that is a *fight*. `combat::hostile_actors`
/// is this set over the declared actors.
pub fn unleashed_actors(c: &Campaign) -> BTreeSet<&str> {
    let mut out = BTreeSet::new();
    for_each_campaign_effect(c, &mut |_, _, eff| {
        if let Verb::UnleashActor { actor, .. } = &eff.verb {
            out.insert(actor.as_str());
        }
    });
    out
}

/// The area a stage-4 quest belongs to.
pub fn quest_area<'a>(c: &'a Campaign, quest_id: &str) -> Option<&'a str> {
    c.quest_plan
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == quest_id)
        .map(|q| q.area.as_str())
}

/// Does any effect in `effs`, or anywhere in the trees nested under them, fire a
/// `spawn-wave` for `wave_id`?
///
/// Descends through [`QuestEffect::visit_deep`], so `sequence` steps,
/// `set-checkpoint` `on_respawn`, `bonfire` `on_rest`, `begin-stealth`
/// `on_caught` and `move-npc`/`move-actor` `on_arrive` are all spawn sites — as
/// they already are for emission. A verb the emitter compiles from a nesting site
/// is a verb every consumer scan must see from the same site.
fn fires_wave<'a>(effs: impl IntoIterator<Item = &'a QuestEffect>, wave_id: &str) -> bool {
    let mut found = false;
    for e in effs {
        e.visit_deep(&mut |x| {
            if matches!(x.spawn_wave(), Some(w) if w.as_str() == wave_id) {
                found = true;
            }
        });
    }
    found
}

/// The area a wave's mobs spawn in — resolved from the wave's **spawn site**, not
/// from any `kill` objective. A `spawn-wave` effect (on a quest step, on a quest's
/// completion, on an environment trigger, or in another fight's `on_kill`) is
/// what makes a wave appear; its mobs materialize at `Wave.anchor` resolved in
/// that spawning site's area. This is deliberately independent of objective
/// type so a kill-less "live threat" wave (spec-0008 §4 — e.g. a weakened warden
/// the player sneaks past, an ambient mob flock) resolves a spawn position
/// exactly like a wave that is later slain.
///
/// Resolution order: the quest that fires the `spawn-wave` (`on_objective_complete`
/// or `on_complete`); else, in a single-area campaign, an environment trigger or a
/// trap payload that fires it (both are global — their sole possible area is the
/// one area); else a fight's `on_kill` that fires it (spec-0074) — the area of the
/// wave whose kill fires it, or, for an actor's kill, the sole area of a
/// single-area campaign exactly as for a trigger; else a quest whose `kill`
/// objective references the wave (defensive fallback for a wave declared with a
/// kill but no explicit spawn). `None` if nothing spawns it.
///
/// **Every root is walked DEEP** (`fires_wave`), through
/// [`QuestEffect::nested_effect_lists`] — the DSL's single authority on effect
/// nesting, and the same authority `emit::all_campaign_effects` walks to decide
/// what to compile. A wave the emitter writes a `function <ns>:spawn_<wave>` call
/// for is therefore always a wave this function resolves an area for, and so
/// always a wave whose support machinery is emitted: the agreement is structural,
/// not two walks that have to remember each other.
///
/// It used to be a shallow scan of the top-level chains only, and the island's
/// round-21 build is what that cost: `wave/storm-shore` and `wave/storm-fire` were
/// fired from step 7 of a `sequence`, resolved no area, got no `spawn_…`, no
/// census, no brand and no kill reward — while `seq_under_ram` still shipped the
/// call. Two of three storm waves never spawned (`DW0497` is now the standing
/// proof that this class cannot ship again).
pub fn wave_area<'a>(campaign: &'a Campaign, wave_id: &str) -> Option<&'a str> {
    wave_area_seen(campaign, wave_id, &mut BTreeSet::new())
}

fn wave_area_seen<'a>(
    campaign: &'a Campaign,
    wave_id: &str,
    seen: &mut BTreeSet<String>,
) -> Option<&'a str> {
    if !seen.insert(wave_id.to_string()) {
        // A wave whose only spawn site is its own kill (directly or through a
        // ring of fights) is seated by nothing.
        return None;
    }
    let content = &campaign.quests.content;
    // 1. A quest whose effect TREE fires `spawn-wave` for this wave — the true
    //    spawn site.
    for q in &content.quests {
        if fires_wave(
            q.on_objective_complete
                .values()
                .flatten()
                .chain(&q.on_complete),
            wave_id,
        ) {
            return quest_area(campaign, q.id.as_str());
        }
    }
    let single_area = campaign.world.content.areas.len() == 1;
    let sole_area = || campaign.world.content.areas.first().map(|a| a.id.as_str());
    // 2. An environment trigger or trap payload that fires it. Both are global
    //    effect roots carrying no area of their own; in a single-area campaign the
    //    sole area is unambiguous. (Multi-area trigger-only waves are not
    //    resolvable here and surface as a build diagnostic rather than a silent
    //    dangling spawn.)
    if single_area
        && (content
            .triggers
            .iter()
            .any(|t| fires_wave(&t.effects, wave_id))
            || content
                .traps
                .iter()
                .any(|t| fires_wave(&t.payload, wave_id)))
    {
        return sole_area();
    }
    // 3. A fight's `on_kill` that fires it (spec-0074). A wave's kill plays where
    //    that wave's bodies stand; an actor's kill is global like a trigger.
    for w in &content.waves {
        if let Some(ok) = &w.on_kill
            && fires_wave(&ok.effects, wave_id)
            && let Some(area) = wave_area_seen(campaign, w.id.as_str(), seen)
        {
            return Some(area);
        }
    }
    if single_area
        && content.actors.iter().any(|a| {
            a.on_kill
                .as_ref()
                .is_some_and(|ok| fires_wave(&ok.effects, wave_id))
        })
    {
        return sole_area();
    }
    // 4. Defensive fallback: a `kill` objective's quest.
    for q in &content.quests {
        if q.objectives
            .iter()
            .any(|o| matches!(o, Objective::Kill { wave, .. } if wave.as_str() == wave_id))
        {
            return quest_area(campaign, q.id.as_str());
        }
    }
    None
}
