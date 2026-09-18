//! **Whether a fight comes back, and the `fires` judgement it owes** (spec-0074
//! §4, §8.2, §8.3).
//!
//! A fight *comes back* when the engine re-seats it after the party has met it:
//! a `respawns_on_rest` wave, an undefeated `elite`/`boss` wave and a hostile
//! actor — all three only where the campaign has a rest point — and any fight
//! whose seating beat (`spawn-wave` for a wave, `unleash-actor` for an actor)
//! lies where it can fire more than once. [`fight_comes_back`] is the one
//! derivation; `Plan::fight_comes_back` asks it with the plan's collected rest
//! points, and [`check_on_kill_fires`] asks it before any build, where the rest
//! points are read off the documents.
//!
//! Where a fight comes back, `on_kill.fires` is **required** (`DW0915`): whether
//! an economy can be farmed is the creator's judgement, not the engine's. Where
//! it does not, the two values coincide, so `fires` is optional and `every-kill`
//! is refused as inert (`DW0914`). The two refusals are one pair over one
//! predicate: on a fight that comes back, `every-kill` and `first-kill` pass and
//! an absent `fires` reds; on one that does not, `every-kill` reds and the other
//! two pass.
//!
//! **What "can fire more than once" reads.** The root a seating beat hangs off:
//! an environment trigger with `once: false`, a trap that re-arms, the campaign's
//! `on_death`, a dialogue `on_respawn`, a shop offer and another fight's
//! `on_kill` all fire repeatedly; a quest's bundles and a shortcut's `on_unlock`
//! fire once. A beat nested in a list that fires on its own repeating event — a
//! bonfire's `on_rest`, a checkpoint's `on_respawn`, a stealth `on_caught` —
//! repeats whatever its root does. Two beats that each fire once seat the fight
//! twice, so a second seating beat makes it come back too. The walk is
//! [`for_each_effect_root`] and the
//! nesting authority [`QuestEffect::nested_effect_lists_labeled`], so a new root
//! is a compile error in [`root_repeats`] rather than a quiet `false`.

use delvewright_dsl::{
    Campaign, Diagnostic, DwCode, EffectRootOwner, EncounterTier, ExitTier, Fight, KillFires,
    QuestEffect, TrapReset, Verb, for_each_effect_root,
};

/// `DW0914`: `fires: every-kill` on a fight that never comes back — the two
/// values coincide there, so the declaration binds to nothing. Validation-tier
/// (exit 1).
pub const DW_ON_KILL_EVERY_INERT: DwCode = DwCode::new("DW0914", ExitTier::Build);

/// `DW0915`: an `on_kill` with no `fires` on a fight that comes back — whether a
/// body that comes back pays again is the creator's judgement. Validation-tier
/// (exit 1).
pub const DW_ON_KILL_FIRES_OWED: DwCode = DwCode::new("DW0915", ExitTier::Build);

/// Why a fight comes back after the party has met it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComesBack {
    /// A `respawns_on_rest` wave: every rest at a bonfire re-seats it, beaten or
    /// not.
    RespawnsOnRest,
    /// A wave billed `elite`/`boss`: a rest re-seats it while it still stands.
    Undefeated(EncounterTier),
    /// An unleashed actor: a rest re-stands it while it still stands.
    Unleashed,
    /// The beat that seats it can fire more than once.
    Repeats {
        /// JSON pointer to the seating beat, in the quests or dialogue document.
        path: String,
        /// Why that beat can fire again, in the engine's words.
        why: String,
    },
}

impl ComesBack {
    /// The site that makes the fight come back, as a `DW0915` message names it.
    pub fn describe(&self) -> String {
        match self {
            ComesBack::RespawnsOnRest => {
                "it declares `respawns_on_rest: true` and the campaign has a `bonfire`, so \
                 every rest re-seats it whether or not it was beaten"
                    .to_string()
            }
            ComesBack::Undefeated(t) => format!(
                "it is billed `tier: {}` and the campaign has a `bonfire`, so a rest re-seats \
                 it while any of it still stands",
                t.token()
            ),
            ComesBack::Unleashed => {
                "an `unleash-actor` turns it loose and the campaign has a `bonfire`, so a rest \
                 re-stands it while it still stands"
                    .to_string()
            }
            ComesBack::Repeats { path, why } => {
                format!("the beat that seats it at `{path}` can fire more than once ({why})")
            }
        }
    }
}

/// Does `fight` come back after the party has met it — and if so, why?
///
/// `rests` is whether the campaign has a rest point: the plan's collected
/// bonfires at build time ([`crate::compiler::plan::Plan::fight_comes_back`]),
/// [`delvewright_dsl::declares_bonfire`] before any build. The rest re-seats are
/// the ones the bonfire emits (`Plan::reseat_waves`,
/// `Plan::undefeated_reseat_waves`, `Plan::reseat_actors`), read here by the
/// same declarations; the repeat is the first seating beat that can fire again.
pub fn fight_comes_back(c: &Campaign, rests: bool, fight: Fight<'_>) -> Option<ComesBack> {
    if rests {
        match fight {
            Fight::Wave(w) if w.respawns_on_rest => return Some(ComesBack::RespawnsOnRest),
            Fight::Wave(w) => {
                if let Some(t) = w.tier.filter(|t| t.has_floor_expectation()) {
                    return Some(ComesBack::Undefeated(t));
                }
            }
            Fight::Actor(a) => {
                if delvewright_dsl::unleashed_actors(c).contains(a.id.as_str()) {
                    return Some(ComesBack::Unleashed);
                }
            }
        }
    }
    repeating_seat(c, fight)
}

/// Is `eff` the beat that seats `fight`?
fn seats(eff: &QuestEffect, fight: Fight<'_>) -> bool {
    match fight {
        Fight::Wave(w) => eff.spawn_wave().is_some_and(|id| id == &w.id),
        Fight::Actor(a) => matches!(&eff.verb, Verb::UnleashActor { actor, .. } if actor == &a.id),
    }
}

/// Whether a root fires more than once, and why — exhaustive over the owner, so
/// a new root answers here or fails to compile.
fn root_repeats(owner: &EffectRootOwner<'_>) -> Option<&'static str> {
    match owner {
        EffectRootOwner::ObjectiveComplete { .. }
        | EffectRootOwner::QuestComplete { .. }
        | EffectRootOwner::ShortcutUnlock(_) => None,
        EffectRootOwner::Trigger(t) => {
            (!t.once).then_some("an environment trigger with `once: false`")
        }
        EffectRootOwner::TrapPayload(t) => {
            (t.reset == TrapReset::Rearm).then_some("a trap that re-arms (`reset: rearm`)")
        }
        EffectRootOwner::DialogueRespawn => {
            Some("a checkpoint's `on_respawn`, run on every respawn")
        }
        EffectRootOwner::OnDeath => Some("the campaign's `on_death`, run on every death"),
        EffectRootOwner::ShopOffer(_) => Some("a shop offer, run each time it is bought"),
        EffectRootOwner::OnKill(_) => Some("a fight's `on_kill`, run for each credited kill"),
    }
}

/// Whether a nested list fires on a repeating event of its own, whatever its
/// root does — keyed by the path segment the nesting authority labels it with.
fn nested_repeats(segment: &str) -> Option<&'static str> {
    match segment {
        "on_rest" => Some("a bonfire's `on_rest`, run on every rest"),
        "on_respawn" => Some("a checkpoint's `on_respawn`, run on every respawn"),
        "on_caught" => Some("a stealth `on_caught`, run each time the party is seen"),
        _ => None,
    }
}

/// The seating beat that makes `fight` come back without a rest: the first one
/// that can fire more than once, or — where every beat fires once — a second
/// beat that seats it again.
fn repeating_seat(c: &Campaign, fight: Fight<'_>) -> Option<ComesBack> {
    /// Every seating beat, in walk order, with why it repeats (if it does).
    fn deep(
        eff: &QuestEffect,
        path: &str,
        repeats: Option<&'static str>,
        fight: Fight<'_>,
        out: &mut Vec<(String, Option<&'static str>)>,
    ) {
        if seats(eff, fight) {
            out.push((path.to_string(), repeats));
        }
        for (seg, _key, list) in eff.nested_effect_lists_labeled() {
            let first = seg.split('/').next().unwrap_or_default();
            let inner = repeats.or(nested_repeats(first));
            for (j, e) in list.iter().enumerate() {
                deep(e, &format!("{path}/{seg}/{j}"), inner, fight, out);
            }
        }
    }
    let mut beats = Vec::new();
    for_each_effect_root(c, &mut |site, list| {
        let repeats = root_repeats(&site.owner);
        for (i, eff) in list.iter().enumerate() {
            deep(
                eff,
                &format!("{}/{i}", site.path),
                repeats,
                fight,
                &mut beats,
            );
        }
    });
    if let Some((path, Some(why))) = beats.iter().find(|(_, r)| r.is_some()) {
        return Some(ComesBack::Repeats {
            path: path.clone(),
            why: why.to_string(),
        });
    }
    match beats.as_slice() {
        [first, second, ..] => Some(ComesBack::Repeats {
            path: second.0.clone(),
            why: format!(
                "a second beat seats it again; the first is at `{}`",
                first.0
            ),
        }),
        _ => None,
    }
}

/// What a fight lacks to come back, as a `DW0914` message names it.
fn lacks(fight: Fight<'_>, rests: bool) -> String {
    let rest = if rests {
        ""
    } else {
        "the campaign has no `bonfire`, so no rest re-seats anything; "
    };
    match fight {
        Fight::Wave(_) => format!(
            "{rest}it is not `respawns_on_rest`, it is not billed `elite`/`boss`, and no beat \
             that can fire more than once spawns it"
        ),
        Fight::Actor(_) => format!(
            "{rest}no beat that can fire more than once unleashes it{}",
            if rests {
                ", and no `unleash-actor` turns it loose for a rest to re-stand"
            } else {
                ""
            }
        ),
    }
}

/// `DW0914` / `DW0915` over every fight that declares an `on_kill` (spec-0074
/// §8.2, §8.3), before any build: the rest points are read off the documents
/// ([`delvewright_dsl::declares_bonfire`]). A campaign with no bundle gets
/// nothing.
pub fn check_on_kill_fires(c: &Campaign) -> Vec<Diagnostic> {
    let rests = delvewright_dsl::declares_bonfire(c);
    let mut d = Vec::new();
    for (path, fight) in delvewright_dsl::fights(c) {
        let Some(ok) = fight.on_kill() else {
            continue;
        };
        let back = fight_comes_back(c, rests, fight);
        match (&back, ok.fires) {
            (Some(why), None) => d.push(Diagnostic::error(
                DW_ON_KILL_FIRES_OWED,
                "quests",
                format!("{path}/on_kill"),
                format!(
                    "`on_kill` on {} `{}` states no `fires`, and this fight comes back: {}. \
                     Whether a body that comes back pays again is your judgement — state \
                     `fires: first-kill` (the fight pays as many kills as it has bodies, once) \
                     or `fires: every-kill` (every kill pays, so the fight can be farmed).",
                    fight.word(),
                    fight.id(),
                    why.describe()
                ),
            )),
            (None, Some(KillFires::EveryKill)) => d.push(Diagnostic::error(
                DW_ON_KILL_EVERY_INERT,
                "quests",
                format!("{path}/on_kill/fires"),
                format!(
                    "`on_kill` on {} `{}` states `fires: every-kill`, but this fight never \
                     comes back — {} — so every kill of it is already a first kill and the \
                     declaration binds to nothing. State `fires: first-kill` or leave `fires` \
                     off, or make the fight come back (a `bonfire` and `respawns_on_rest`, a \
                     tier, or a seating beat that can fire more than once).",
                    fight.word(),
                    fight.id(),
                    lacks(fight, rests)
                ),
            )),
            _ => {}
        }
    }
    d
}

/// What the `on_kill` rules examined on a campaign (spec-0074): fights carrying
/// a bundle over fights declared, how many of those come back, and the refusals.
/// A rule over "every bundle" that bound to none reports its zero rather than
/// passing silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnKillBinding {
    /// Fights (waves + actors) the campaign declares — the denominator.
    pub fights: usize,
    /// Of those, the ones that declare an `on_kill`.
    pub bundles: usize,
    /// Of the bundles, the ones on a fight that comes back.
    pub come_back: usize,
    /// Of the bundles, the ones stating `fires: every-kill`.
    pub every_kill: usize,
    /// `DW0913` + `DW0914` + `DW0915` refusals raised.
    pub refused: usize,
}

impl OnKillBinding {
    /// Count what the `on_kill` rules examine on `c`.
    pub fn of(c: &Campaign) -> Self {
        let rests = delvewright_dsl::declares_bonfire(c);
        let all = delvewright_dsl::fights(c);
        let mut b = OnKillBinding {
            fights: all.len(),
            bundles: 0,
            come_back: 0,
            every_kill: 0,
            refused: 0,
        };
        for (_, f) in &all {
            let Some(ok) = f.on_kill() else { continue };
            b.bundles += 1;
            b.come_back += usize::from(fight_comes_back(c, rests, *f).is_some());
            b.every_kill += usize::from(ok.fires == Some(KillFires::EveryKill));
        }
        let mut d = Vec::new();
        delvewright_dsl::on_kill_checks(c, &mut d);
        d.extend(check_on_kill_fires(c));
        b.refused = d
            .iter()
            .filter(|x| {
                x.code == delvewright_dsl::codes::ON_KILL_UNREACHABLE
                    || x.code == DW_ON_KILL_EVERY_INERT
                    || x.code == DW_ON_KILL_FIRES_OWED
            })
            .count();
        b
    }

    /// The one-line rendering printed under the validation report.
    pub fn line(&self) -> String {
        format!(
            "on_kill binding: {} of {} fight(s) carry a bundle, {} of them on a fight that comes \
             back, {} stating `every-kill`; {} refused (DW0913/DW0914/DW0915).",
            self.bundles, self.fights, self.come_back, self.every_kill, self.refused
        )
    }
}
