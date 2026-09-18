//! **A kill pays** (spec-0074): what happens each time a body of a fight is
//! killed.
//!
//! `on_kill` is an optional property of `waves[]` and `actors[]` — one type on
//! both. Its `effects` run once per body a player is **credited** with killing
//! (vanilla's `minecraft:player_killed_entity`), as that player, so a
//! `player`-scoped datum pays the killer and a `party`-scoped one pays the party
//! once. It is effect root **R9** ([`crate::EffectRootKind::OnKill`]).
//!
//! `fires` is the creator's judgement on whether a body that comes back pays
//! again. It is required exactly where the fight comes back after the party has
//! met it (`DW0915`) and refused as inert where it does not (`DW0914`); both
//! refusals read one compiler-side predicate (`plan::fight_comes_back`), which
//! needs the campaign's rest points.
//!
//! The types ([`crate::stages::OnKill`], [`crate::stages::KillFires`]) are
//! stage-5 surface and live with the rest of it in [`crate::stages`]; this
//! module holds the two refusals that need only the documents:
//!
//! * `DW0100` — an empty `effects` list (the exported schema's `minItems: 1`,
//!   which serde does not enforce);
//! * `DW0913` — a bundle on a body no player can be credited with killing: a
//!   wave no beat spawns ([`crate::fight::wave_area`] is `None`), or an actor no
//!   `unleash-actor` names that is not `vulnerable`
//!   ([`crate::fight::unleashed_actors`]).

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::fight::{Fight, fights, unleashed_actors, wave_area};

/// The document-tier `on_kill` refusals (spec-0074 §8.1): `DW0100` for an empty
/// `effects` list and `DW0913` for a bundle no credited kill can ever reach. A
/// campaign that declares no bundle gets nothing.
pub fn on_kill_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let unleashed = unleashed_actors(c);
    for (path, fight) in fights(c) {
        let Some(ok) = fight.on_kill() else {
            continue;
        };
        if ok.effects.is_empty() {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                "quests",
                format!("{path}/on_kill/effects"),
                format!(
                    "`on_kill` on {} `{}` has no effects. The schema requires at least one \
                     (`minItems: 1`): a bundle that does nothing is not a bundle. Add the effect \
                     each kill should have, or remove `on_kill`.",
                    fight.word(),
                    fight.id()
                ),
            ));
        }
        let unreachable = match fight {
            Fight::Wave(w) => wave_area(c, w.id.as_str()).is_none().then(|| {
                format!(
                    "no beat spawns wave `{}` — no `spawn-wave` names it where the engine can \
                     seat it, so it has no bodies and no kill can be credited. Spawn it from a \
                     beat, or remove the bundle.",
                    w.id
                )
            }),
            Fight::Actor(a) => (!a.vulnerable && !unleashed.contains(a.id.as_str())).then(|| {
                format!(
                    "actor `{}` is never unleashed and is not `vulnerable`, so its body is \
                     `Invulnerable` for the whole delve and no player can be credited with \
                     killing it. Unleash it with an `unleash-actor` beat, mark it \
                     `vulnerable: true`, or remove the bundle.",
                    a.id
                )
            }),
        };
        if let Some(why) = unreachable {
            d.push(Diagnostic::error(
                codes::ON_KILL_UNREACHABLE,
                "quests",
                format!("{path}/on_kill"),
                format!(
                    "`on_kill` on {} `{}` can never fire: {why}",
                    fight.word(),
                    fight.id()
                ),
            ));
        }
    }
}
