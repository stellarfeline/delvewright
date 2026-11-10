//! **A body enters the way its kind does** (spec-0132): the registry of kind
//! actions, the `actors[].entrance` declaration that plays one, and the
//! refusals that keep the declarations about the free body on a body that is
//! freed.
//!
//! The registry is data, not a rule about kinds. A record is keyed by `(kind,
//! action)` — a vanilla entity id and what the body does — and states, for the
//! emitter and the author alike, what the action is in a player's eyes, the
//! vanilla mechanism as the NBT the twin's summon carries, its length in ticks,
//! what the body can and cannot do while it runs, what it ends with, and where
//! in `docs/reference/` it was measured. This module ships the action
//! [`ENTER`] for two kinds and no other row; a kind absent from the table has
//! no native entrance as far as the engine knows, and `DW1081` says so by
//! name.
//!
//! Only the free body plays an entrance: the twin `unleash-actor` summons is
//! the one free-AI body the compiler summons, and a `NoAI` puppet keeps
//! whatever entrance data it was summoned with and never consumes it
//! (`docs/reference/native-entrance.md` §1, §3). So an entering actor is never
//! a puppet (`DW1083`), and a declaration about the free body on an actor
//! nothing frees is refused (`DW1082`).
//!
//! Determinism (ADR-0006): the table is a slice in a fixed order, and every
//! walk is over slices and `BTreeSet`s.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, DwCode, ExitTier};
use crate::envelope::Campaign;
use crate::{Actor, Verb};

/// How an actor's body arrives in the world with its will (spec-0132 §2.1).
///
/// One value, a declaration of intent and not a knob: the engine looks the
/// kind up in [`KIND_ACTIONS`] and plays what it finds. An author who wants a
/// different arrival for a kind that has one writes a different kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Entrance {
    /// The body enters the way its kind does in vanilla — a warden digs out of
    /// the ground, a wither charges up.
    Native,
}

impl Entrance {
    /// The kebab token (`native`).
    pub fn token(self) -> &'static str {
        match self {
            Entrance::Native => "native",
        }
    }
}

/// The action a body performs as it arrives: the one action whose moment the
/// engine fixes (the arrival) rather than a beat (spec-0132 §8).
pub const ENTER: &str = "enter";

/// The vanilla mechanism a kind action is played by, as the data the free
/// body's summon carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mechanism {
    /// One brain memory with an expiry, written into the summon's
    /// `Brain.memories` compound beside any memory the compiler already seeds
    /// there (one `Brain` key: SNBT keeps the last of two keys with one name).
    BrainMemory {
        /// The memory's namespaced id, e.g. `minecraft:is_emerging`.
        memory: &'static str,
        /// Its `ttl`, in ticks.
        ttl: u32,
    },
    /// The spawn charge-up: `Invul` set to the charge's length, and `Health`
    /// at a third of the body's maximum (vanilla's own start).
    ChargeUp {
        /// `Invul`, in ticks.
        invul: u32,
        /// The kind's vanilla `max_health`, read when the actor declares none.
        base_max_health: f64,
    },
}

/// One record of the registry: what a kind does for one action, measured on
/// the pinned server (spec-0132 §2.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KindAction {
    /// The vanilla entity id, e.g. `minecraft:warden`.
    pub kind: &'static str,
    /// What the body does, e.g. [`ENTER`].
    pub action: &'static str,
    /// What the action is in a player's eyes.
    pub seen: &'static str,
    /// The mechanism, as the data the twin's summon carries.
    pub mechanism: Mechanism,
    /// How long it lasts, in ticks: the **entrance window** the harness judges
    /// a pursuer's will after, and what a cutscene author sizes a shot by.
    pub ticks: u32,
    /// What the body can and cannot do while it runs.
    pub while_runs: &'static str,
    /// What it ends with.
    pub ends_with: &'static str,
    /// Where in `docs/reference/` it was measured.
    pub read_site: &'static str,
}

/// **The registry of kind actions** (spec-0132 §2.2), in a fixed order.
pub const KIND_ACTIONS: &[KindAction] = &[
    KindAction {
        kind: "minecraft:warden",
        action: ENTER,
        seen: "digs out of the ground (the EMERGING pose)",
        mechanism: Mechanism::BrainMemory {
            memory: "minecraft:is_emerging",
            ttl: 133,
        },
        ticks: 133,
        while_runs: "does not move, hears nothing, cannot be hurt; a seeded anger is kept",
        ends_with: "stands and sniffs; a kept anger opens the hunt",
        read_site: "docs/reference/warden-behaviour.md §6",
    },
    KindAction {
        kind: "minecraft:wither",
        action: ENTER,
        seen: "the spawn charge-up, the boss bar filling",
        mechanism: Mechanism::ChargeUp {
            invul: 220,
            base_max_health: 300.0,
        },
        ticks: 220,
        while_runs: "does not move, fires no skull, cannot be hurt; heals 10 every 10 ticks to full",
        ends_with: "a blast of power 7: breaks no block under the delve's `mob_griefing false`; \
                    about 51 damage to an unarmoured body at 3 blocks and 40 at 6",
        read_site: "docs/reference/native-entrance.md §1",
    },
];

/// The record for `(kind, action)`, or `None` when the registry has none.
/// `kind` may be written with or without the `minecraft:` namespace.
pub fn kind_action(kind: &str, action: &str) -> Option<&'static KindAction> {
    let kind = kind.strip_prefix("minecraft:").unwrap_or(kind);
    KIND_ACTIONS
        .iter()
        .find(|r| r.action == action && r.kind.strip_prefix("minecraft:") == Some(kind))
}

/// Every kind the registry has a record of `action` for, in table order.
pub fn kinds_with(action: &str) -> Vec<&'static str> {
    KIND_ACTIONS
        .iter()
        .filter(|r| r.action == action)
        .map(|r| r.kind)
        .collect()
}

/// **The entrance an actor's free body plays**: the [`ENTER`] record of the
/// twin's kind — `entity` as written, since a `skin` dresses only the puppet —
/// when the actor declares `entrance: "native"`, or `None`. The one answer
/// the emitter, the exported path and the checks read.
pub fn native_entrance(a: &Actor) -> Option<&'static KindAction> {
    a.entrance?;
    kind_action(&a.entity, ENTER)
}

/// The entrance window of an actor's free body, in ticks: the registry's
/// length for its kind, or `0` for a body that declares no `entrance`.
pub fn entrance_ticks(a: &Actor) -> u32 {
    native_entrance(a).map_or(0, |r| r.ticks)
}

crate::dw_code! {
    /// `DW1081` (spec-0132 §4): **an entrance its kind does not have.** An
    /// actor declares `entrance: "native"` and the registry holds no `enter`
    /// record for the twin's kind (`entity` as written). Names the actor, the
    /// kind, the action and every kind that has one.
    pub const DW_ENTRANCE_UNKNOWN_KIND: DwCode = DwCode::new("DW1081", ExitTier::Build);
}

crate::dw_code! {
    /// `DW1082` (spec-0132 §4): **a declaration about the free body on an
    /// actor the free body never comes to.** An `entrance` or an `outrun` on
    /// an actor no `unleash-actor` names anywhere (raised by `dsl::validate`);
    /// and an `outrun` whose actor is loose on no step of the exported critical
    /// path (raised by the compiler, where the path's pursuers are derived).
    pub const DW_FREE_BODY_NEVER_FREED: DwCode = DwCode::new("DW1082", ExitTier::Build);
}

crate::dw_code! {
    /// `DW1083` (spec-0132 §4): **a puppet surface on an actor that enters.**
    /// A `spawn-actor` or `move-actor` naming it anywhere, or `vulnerable`,
    /// `watch` or `answers` on it — each a property of a body that stands
    /// caged, which an entering body never does.
    pub const DW_ENTERING_BODY_PUPPETED: DwCode = DwCode::new("DW1083", ExitTier::Build);
}

/// The stage a JSON pointer into the campaign belongs to.
fn stage_of(path: &str) -> &'static str {
    if path.starts_with("/content/dialogues") {
        "dialogue"
    } else {
        "quests"
    }
}

/// `DW1081`, `DW1082`'s first arm and `DW1083` over every actor (spec-0132
/// §4). Run by `dsl::validate` through [`crate::actor::actor_checks`].
pub(crate) fn entrance_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let actors = &c.quests.content.actors;
    if !actors.iter().any(|a| a.entrance.is_some() || a.outrun) {
        return;
    }
    let unleashed: BTreeSet<&str> = crate::fight::unleashed_actors(c);
    let entering: BTreeSet<&str> = actors
        .iter()
        .filter(|a| a.entrance.is_some())
        .map(|a| a.id.as_str())
        .collect();
    for (i, a) in actors.iter().enumerate() {
        let id = a.id.as_str();
        if let Some(e) = a.entrance
            && native_entrance(a).is_none()
        {
            let kinds = kinds_with(ENTER)
                .iter()
                .map(|k| format!("`{k}`"))
                .collect::<Vec<_>>()
                .join(", ");
            d.push(Diagnostic::error(
                DW_ENTRANCE_UNKNOWN_KIND,
                "quests",
                format!("/content/actors/{i}/entrance"),
                format!(
                    "actor `{id}` declares `entrance: \"{}\"`, but its kind `{}` has no `{ENTER}` \
                     action the engine knows — the kinds that enter are {kinds}. Drop the field, \
                     or give the body a kind that enters",
                    e.token(),
                    a.entity
                ),
            ));
        }
        if !unleashed.contains(id) {
            for (field, set) in [("entrance", a.entrance.is_some()), ("outrun", a.outrun)] {
                if !set {
                    continue;
                }
                d.push(Diagnostic::error(
                    DW_FREE_BODY_NEVER_FREED,
                    "quests",
                    format!("/content/actors/{i}/{field}"),
                    format!(
                        "actor `{id}` declares `{field}`, a property of its free body, and no \
                         `unleash-actor` anywhere in the campaign names it, so that body never \
                         comes to the world (no unleash names it). Unleash the actor on the path, \
                         or drop the field"
                    ),
                ));
            }
        }
        if a.entrance.is_none() {
            continue;
        }
        for (field, set) in [
            ("vulnerable", a.vulnerable),
            ("watch", a.watch.is_some()),
            ("answers", a.answers.is_some()),
        ] {
            if !set {
                continue;
            }
            d.push(Diagnostic::error(
                DW_ENTERING_BODY_PUPPETED,
                "quests",
                format!("/content/actors/{i}/{field}"),
                format!(
                    "actor `{id}` enters (`entrance` is declared) and also declares `{field}`, a \
                     property of a body that stands caged as a puppet; an entering body is never \
                     a puppet — its first appearance is its `unleash-actor`. Remove `{field}`"
                ),
            ));
        }
    }
    crate::effects::for_each_campaign_effect(c, &mut |path, _site, e| {
        let (verb, actor) = match &e.verb {
            Verb::SpawnActor { actor } => ("spawn-actor", actor),
            Verb::MoveActor { actor, .. } => ("move-actor", actor),
            _ => return,
        };
        if !entering.contains(actor.as_str()) {
            return;
        }
        d.push(Diagnostic::error(
            DW_ENTERING_BODY_PUPPETED,
            stage_of(path),
            path.to_string(),
            format!(
                "a `{verb}` at {path} names actor `{actor}`, which enters (`entrance` is \
                 declared): an entering body is never a puppet, so there is nothing to {}. \
                 Remove the `{verb}`; the body's first appearance is its `unleash-actor`",
                if verb == "spawn-actor" {
                    "stand before it arrives"
                } else {
                    "walk before it arrives"
                }
            ),
        ));
    });
}
