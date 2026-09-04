//! Daylight-burning staging proof: a body the sun kills may not be staged where
//! the sun can reach it (`DW0496`).
//!
//! ## The defect this exists for (`hollow-vigil`)
//!
//! The walls-down round carved the gate yard's roof and two of its walls open to
//! the sky. The world is pinned `time set noon`. The first zombie wave musters a
//! short walk from that yard. Chased out of the keep, the footmen burned — two of
//! three dead to sunlight in under twenty seconds, at `x=6.3 z=-3.2` and
//! `x=3.0 z=-8.4`, both outside the carved north wall — so the beat the party was
//! supposed to *fight* was settled by the weather.
//!
//! Every rung of the ladder was green. `DW0312` proved the wave had footing;
//! `DW0311` proved the room was reachable; spec-0023 proved the fight was
//! winnable; the liveness census made sure a wave that dies to *anything*
//! still closes its objective — which answers the soft-lock and, deliberately,
//! not the encounter. Nothing there relates "this body burns in daylight" to "this
//! is a fight the party is meant to have".
//!
//! ## The rule
//!
//! A staged combatant is `DW0496` when all five hold:
//!
//! 1. **It burns.** Its entity type is in vanilla's own
//!    `#minecraft:burn_in_daylight` tag and is not fire-immune ([`burns_in_daylight`]).
//! 2. **It is a fight.** A `kill` objective adjudicates its wave, or it is an
//!    actor the party can actually damage ([`fightable_actor`]).
//! 3. **The sun is up.** The delve's declared time/weather leave the burn tick
//!    live for the whole delve ([`daylight_is_pinned`]).
//! 4. **The sun can reach it.** Open sky stands on ground it can walk to, within
//!    one aggro radius of where it is staged ([`sky_within_reach`]).
//! 5. **Nothing on its head.** No `equipment.head` — except for a phantom, whose
//!    burn a helmet does not stop (see below).
//!
//! ### 1. Which bodies burn — Mojang's list, never ours
//!
//! `#minecraft:burn_in_daylight` is a **built-in vanilla `entity_type` tag**, and
//! since 1.21 it is the thing the engine itself tests before running a mob's
//! sun-burn tick. It is vendored verbatim from Mojang's generated reports
//! (`crates/dsl/data/entity-tags-1.21.11.json`, `data/PROVENANCE.md`), so the question "does
//! this species burn?" is answered by the game, not by a species table the
//! compiler invented — the refusal this codebase already makes for mob health
//! (`DW0475`) and aggro range ([`crate::nav::DEFAULT_FOLLOW_RANGE`]).
//!
//! For 1.21.11 the tag holds `skeleton`, `stray`, `bogged`, `wither_skeleton`,
//! `zombie`, `zombie_villager`, `zombie_horse`, `drowned`, `zombie_nautilus`,
//! `phantom` — and, tellingly, **not** `husk` or `zombified_piglin`, the two
//! everybody remembers as exceptions.
//!
//! The tag says which types *run* the burn tick, not which types the resulting
//! fire *hurts*. Fire immunity is a hardcoded entity-type property that appears in
//! no vanilla data branch at all, and exactly one member of the tag has it:
//! `minecraft:wither_skeleton`, a Nether native ("The notable exceptions to this
//! are the Nether-native undead mobs, which are entirely immune to fire" —
//! [Minecraft Wiki, *Undead*](https://minecraft.wiki/w/Undead)). That single
//! exclusion is [`FIRE_IMMUNE`], stated here rather than smuggled into the data.
//!
//! ### 5. Why a helmet is the answer, and where it is not
//!
//! Vanilla's burn tick checks the head slot first: a mob wearing head armour
//! damages the helmet instead of igniting ("wearing head armor (the helmet has a
//! 50% chance to lose 1 durability for every tick the zombie would normally be set
//! on fire)" — [Minecraft Wiki, *Zombie*](https://minecraft.wiki/w/Zombie)). That
//! is why `equipment.head` is the owner's sanctioned remedy, recorded on the DSL
//! field itself, and why `set-time` never is: the delve's hour is a *pacing*
//! decision, and moving it to save a mob spends a beat the author authored.
//!
//! `minecraft:phantom` is the exception, and it is explicit: "Like zombies and
//! skeletons, phantoms burn in sunlight. They burn even when equipped with helmets
//! through commands" ([Minecraft Wiki, *Phantom*](https://minecraft.wiki/w/Phantom)).
//! So the head slot is no exemption for a phantom and the diagnostic must not
//! offer one — prescribing a fix that does not work is worse than not firing.
//!
//! ### 4. How far the sun counts as "reaching" it
//!
//! The mob does not have to be standing in the light when it spawns. It holds its
//! target while the player stays inside its `follow_range`, so a retreating player
//! drags it exactly as far as the player walks — which is what happened at
//! Barrowmere. The compiler does not model a moving chase, so it asks the weaker,
//! decidable question:
//!
//! > is there open sky **within one aggro radius** of where this thing stands, on
//! > ground it can **walk to**?
//!
//! One radius is the shortest lure that provably exists: a player standing there
//! is inside the mob's perception, and the mob's route to them is a route the
//! compiler has already proven walkable. A longer lure works too, so this
//! *under*-fires by construction and never invents a defect.
//!
//! * **Radius** — the stack's declared `attributes.follow_range`, else
//!   [`crate::nav::DEFAULT_FOLLOW_RANGE`]: one documented number, never a
//!   per-species table, exactly as `DW0380`'s optional-elite spheres and
//!   `DW0478`'s bonfire aggro test read it.
//! * **Walkable** — [`crate::nav::World::reachable_walkable`] from the seated
//!   spawn cells over the assembled (and stage-7 edited) world, unbounded: getting
//!   there is a question about geometry, not about perception. Bounding the *walk*
//!   by the radius would have been green on the incident — the yard is 15.6 blocks
//!   from the muster room but 21 steps of corridor away.
//! * **Open sky** — [`crate::light::LightModel::sky_open`], the same geometric
//!   column test spec-0010's relight seeds sky light with. One model of "the sky is
//!   above this cell" in the compiler, not two.
//!
//! ### 3. Why the campaign-wide time reading is sound
//!
//! The daylight cycle is **frozen** (`advance_time false`, spec-0010): the declared
//! state persists until a `set-time` cuts to another. So a campaign that declares a
//! burning hour and never cuts is burning at every beat — no per-quest timeline
//! needed. A campaign that *does* cut its time or weather has a beat-by-beat
//! schedule this proof does not model, and it stays silent there rather than guess
//! (again: withhold, never invent). [`daylight_is_pinned`].
//!
//! ## Prescription
//!
//! Put a helmet on the stack (`equipment.head`, drop chance 0 is emitted for you),
//! or roof the ground the fight happens on. Never `set-time`.
//!
//! ## Known boundary
//!
//! Waves a `kill` objective adjudicates, and actors the party can damage. A wave
//! nobody is asked to kill — ambience, a live threat walking a lane — is a
//! difficulty question rather than a broken encounter, and is not this rule's
//! business. Flight is not modelled: a phantom is tested over walkable ground,
//! which can only under-fire for a body that can also fly to the sky.

use crate::failure::Failure;
use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{Campaign, Objective, WorldTime, WorldWeather};

use crate::light::LightModel;
use crate::nav::{DEFAULT_FOLLOW_RANGE, World};
use crate::plan::Plan;
use delvewright_dsl::{DwCode, ExitTier};

/// `DW0496`: a body vanilla burns in daylight is staged for a fight whose ground
/// reaches open sky, in a delve pinned to a burning hour, with nothing on its
/// head.
pub const DW_DAYLIGHT_BURNS_STAGING: DwCode = DwCode::every_version("DW0496", ExitTier::Build);

/// Vanilla's built-in daylight-burn tag, vendored from Mojang's generated
/// reports (`crates/dsl/data/entity-tags-1.21.11.json`; `data/PROVENANCE.md`).
const BURN_IN_DAYLIGHT_TAG: &str = "minecraft:burn_in_daylight";

/// The one member of `#minecraft:burn_in_daylight` the fire cannot hurt.
///
/// The tag names the types that RUN the sun-burn tick; fire immunity is a
/// hardcoded entity-type property that no vanilla data branch publishes, so this
/// exclusion is stated here with its citation rather than implied by the data.
/// "The notable exceptions to this are the Nether-native undead mobs, which are
/// entirely immune to fire" — <https://minecraft.wiki/w/Undead>.
const FIRE_IMMUNE: [&str; 1] = ["minecraft:wither_skeleton"];

/// The one member whose burn a helmet does NOT stop.
///
/// "Like zombies and skeletons, phantoms burn in sunlight. They burn even when
/// equipped with helmets through commands" — <https://minecraft.wiki/w/Phantom>.
/// Every other burner takes the head-slot durability hit instead of igniting
/// (<https://minecraft.wiki/w/Zombie>), which is what makes `equipment.head` the
/// sanctioned remedy for them and not for this one.
const HELMET_PROOF: [&str; 1] = ["minecraft:phantom"];

/// Whether vanilla burns this entity type in daylight: in
/// `#minecraft:burn_in_daylight` and not [`FIRE_IMMUNE`].
///
/// The vendored tag table now lives in [`crate::registry`] — it is vanilla
/// registry data, and `DW0452`/`DW0453` read it too.
pub fn burns_in_daylight(entity: &str) -> bool {
    !FIRE_IMMUNE.contains(&crate::registry::namespaced_entity(entity).as_str())
        && crate::registry::entity_in_tag(entity, BURN_IN_DAYLIGHT_TAG)
}

/// Whether a helmet stops this entity type's burn (everything except a phantom).
fn helmet_helps(entity: &str) -> bool {
    !HELMET_PROOF.contains(&crate::registry::namespaced_entity(entity).as_str())
}

/// Whether a `(time, weather)` state runs the sun-burn tick at a sky-open cell.
///
/// * Time — only `day` and `noon` count. `dusk` (12000) is the sun already going
///   down and `dawn` (23000) is still before sunrise; both sit at the night floor
///   in [`crate::light::effective_sky`] for the same reason, and holding them
///   non-burning is the conservative reading in both proofs.
/// * Weather — only `clear` counts. Vanilla's `isSunBurnTick` is gated on the mob
///   not being in water or rain, so a rained-on delve does not burn its undead at
///   all.
fn state_burns(time: WorldTime, weather: WorldWeather) -> bool {
    matches!(time, WorldTime::Day | WorldTime::Noon) && matches!(weather, WorldWeather::Clear)
}

/// Whether the delve is pinned to a burning hour for its whole length: the
/// declared initial state burns, and so does every state a `set-time` /
/// `set-weather` can reach.
///
/// The daylight cycle is frozen (spec-0010), so the declared state IS the state
/// until something cuts it — which makes the no-cut campaign decidable without a
/// per-beat timeline. A campaign that DOES cut has such a timeline and this
/// returns `false`: withhold rather than invent (the same direction every proof
/// in this module takes).
pub fn daylight_is_pinned(c: &Campaign) -> bool {
    let (times, weathers) = crate::light::reachable_time_weather(c);
    times
        .iter()
        .all(|&t| weathers.iter().all(|&w| state_burns(t, w)))
}

/// The nearest sky-open cell within `radius` blocks of any cell in `from`, on
/// ground walk-reachable from `from` — the shortest lure, the one worth naming
/// in the diagnostic.
///
/// Reachability is unbounded and the radius applies to the sky-open cell only —
/// see the module docs: getting there is geometry, the radius is perception.
/// Deterministic (ADR-0006): `BTreeSet` frontier, integer squared distances, and
/// a total `(d², cell)` tie-break so the named cell never depends on iteration
/// luck.
fn sky_within_reach(
    world: &World,
    light: &LightModel,
    from: &[[i32; 3]],
    radius: u32,
) -> Option<[i32; 3]> {
    let r2 = i64::from(radius) * i64::from(radius);
    let d2 = |cell: [i32; 3]| {
        from.iter()
            .map(|&s| {
                (0..3)
                    .map(|i| i64::from(cell[i] - s[i]).pow(2))
                    .sum::<i64>()
            })
            .min()
            .unwrap_or(i64::MAX)
    };
    world
        .reachable_walkable(from)
        .into_iter()
        .filter(|&cell| d2(cell) <= r2 && light.sky_open(cell))
        .min_by_key(|&cell| (d2(cell), cell))
}

/// The aggro radius of a mob stack: its declared `attributes.follow_range`, else
/// [`DEFAULT_FOLLOW_RANGE`]. The same reading `DW0380` and `DW0478` take.
fn stack_radius(attributes: Option<delvewright_dsl::MobAttributes>) -> u32 {
    attributes
        .and_then(|a| a.follow_range)
        .map(|r| r.max(0.0) as u32)
        .unwrap_or(DEFAULT_FOLLOW_RANGE)
}

/// Every wave id a `kill` objective adjudicates — the waves the party is asked to
/// put down, wherever in the quest graph the objective sits.
fn killed_waves(c: &Campaign) -> BTreeSet<&str> {
    c.quests
        .content
        .quests
        .iter()
        .flat_map(|q| q.objectives.iter())
        .filter_map(|o| match o {
            Objective::Kill { wave, .. } => Some(wave.as_str()),
            _ => None,
        })
        .collect()
}

/// Whether the party can actually fight this actor: a `vulnerable` puppet (the
/// tower-defense creep) or one an `unleash-actor` gives real AI to. A staged
/// `Invulnerable` puppet takes no damage at all, fire included, so it cannot burn
/// and is not this rule's business.
fn fightable_actor(c: &Campaign, actor: &delvewright_dsl::Actor) -> bool {
    if actor.vulnerable {
        return true;
    }
    let mut unleashed = false;
    delvewright_dsl::for_each_campaign_effect(c, &mut |_, _, eff| {
        if let delvewright_dsl::QuestEffect::UnleashActor { actor: id, .. } = eff
            && id.as_str() == actor.id.as_str()
        {
            unleashed = true;
        }
    });
    unleashed
}

/// One staged body the proof looks at.
struct Staged {
    /// `wave/…` or `actor/…`, for the message.
    owner: String,
    /// What the campaign calls the encounter (`wave` / `actor`).
    kind: &'static str,
    /// The vanilla entity id as authored.
    entity: String,
    /// Cells the body is staged on.
    cells: Vec<[i32; 3]>,
    /// Its aggro radius in blocks.
    radius: u32,
    /// Does it already wear something on its head?
    helmeted: bool,
}

/// Prove no daylight-burning body is staged for a fight the sun can reach
/// (`DW0496`).
///
/// `spawns` is the seated wave placement (`emit::plan_wave_spawns`) — the exact
/// cells the datapack will summon on, so the proof measures from where the mobs
/// actually land and not from an anchor they stand around.
pub fn check_daylight_staging(
    plan: &Plan,
    world: &World,
    blocks: &BTreeMap<[i32; 3], String>,
    spawns: &BTreeMap<String, Vec<[i32; 3]>>,
) -> Result<(), Failure> {
    let c = plan.campaign;
    if !daylight_is_pinned(c) {
        return Ok(());
    }
    let staged = collect_staged(plan, spawns);
    if staged.is_empty() {
        return Ok(());
    }
    let light = LightModel::from_blocks(blocks.clone());
    for body in &staged {
        if !burns_in_daylight(&body.entity) {
            continue;
        }
        if body.helmeted && helmet_helps(&body.entity) {
            continue;
        }
        let Some(sunlit) = sky_within_reach(world, &light, &body.cells, body.radius) else {
            continue;
        };
        return Err(Failure {
            code: DW_DAYLIGHT_BURNS_STAGING,
            message: burn_message(body, sunlit),
        });
    }
    Ok(())
}

/// Every staged body worth proving: wave stacks a `kill` objective adjudicates,
/// and actors the party can damage. Deterministic order (declaration order,
/// waves then actors).
fn collect_staged(plan: &Plan, spawns: &BTreeMap<String, Vec<[i32; 3]>>) -> Vec<Staged> {
    let c = plan.campaign;
    let fought = killed_waves(c);
    let mut out: Vec<Staged> = Vec::new();
    for w in &c.quests.content.waves {
        if !fought.contains(w.id.as_str()) {
            continue;
        }
        let Some(cells) = spawns.get(w.id.as_str()) else {
            continue;
        };
        // The seated cells are one flat list in mob-stack order (`plan_wave_spawns`
        // takes `wave_total` of them, stack by stack), so walk them the same way.
        let mut next = 0usize;
        for m in &w.mobs {
            let take = (m.count as usize).min(cells.len().saturating_sub(next));
            let mine = cells[next..next + take].to_vec();
            next += take;
            if mine.is_empty() {
                continue;
            }
            out.push(Staged {
                owner: w.id.as_str().to_string(),
                kind: "wave",
                entity: m.entity.clone(),
                cells: mine,
                radius: stack_radius(m.attributes),
                helmeted: m.equipment.as_ref().is_some_and(|e| e.head.is_some()),
            });
        }
    }
    for a in &c.quests.content.actors {
        if !fightable_actor(c, a) {
            continue;
        }
        let Some(pos) = plan.point_any(a.anchor.as_str()) else {
            continue;
        };
        out.push(Staged {
            owner: a.id.as_str().to_string(),
            kind: "actor",
            entity: a.entity.clone(),
            cells: vec![pos],
            radius: stack_radius(a.attributes),
            helmeted: a.equipment.as_ref().is_some_and(|e| e.head.is_some()),
        });
    }
    out
}

/// The diagnostic text: what is staged, where the sun gets in, and the two fixes
/// — plus the one that is forbidden.
fn burn_message(body: &Staged, sunlit: [i32; 3]) -> String {
    let Staged {
        owner,
        kind,
        entity,
        cells,
        radius,
        helmeted,
    } = body;
    let at = cells[0];
    let head = if *helmeted {
        format!(
            "It already wears a helmet, and for `{entity}` that changes nothing: a phantom \
             burns even when equipped with a helmet through commands (minecraft.wiki/w/Phantom). "
        )
    } else {
        String::new()
    };
    let remedy = if helmet_helps(entity) {
        "Give this stack `equipment.head` (any head item — vanilla damages the helmet instead \
         of igniting the mob, and the compiler emits drop chance 0 so it can never be farmed), \
         or roof the ground the fight happens on."
    } else {
        "Roof the ground the fight happens on, or stage this encounter somewhere the sky does \
         not reach. `equipment.head` is NOT a fix for this species."
    };
    format!(
        "{kind} `{owner}` stages `{entity}` at [{}, {}, {}], and vanilla burns that species in \
         daylight (`#minecraft:burn_in_daylight`). This delve is pinned to a clear daytime hour \
         for its whole length (the daylight cycle is frozen), and open sky stands at \
         [{}, {}, {}] — walkable ground inside this stack's own {radius}-block aggro radius. A \
         player retreating there is still its target, so the fight the party is meant to have \
         is decided by the sun instead: this is the Barrowmere gate yard, where two of three \
         footmen died to sunlight in under twenty seconds with every proof green. {head}Fix the \
         content: {remedy} Do NOT use `set-time` — the delve's hour is a pacing decision the \
         author made, and moving it to save a mob spends a beat; the \
         sanctioned fix is recorded on the `equipment.head` DSL field itself.",
        at[0], at[1], at[2], sunlit[0], sunlit[1], sunlit[2],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tag is Mojang's, and it says what everybody misremembers.
    #[test]
    fn the_vendored_tag_is_the_whole_species_rule() {
        assert!(burns_in_daylight("minecraft:zombie"));
        assert!(burns_in_daylight("minecraft:skeleton"));
        assert!(burns_in_daylight("minecraft:stray"));
        assert!(burns_in_daylight("minecraft:bogged"));
        assert!(burns_in_daylight("minecraft:zombie_villager"));
        assert!(burns_in_daylight("minecraft:drowned"));
        assert!(burns_in_daylight("minecraft:phantom"));
        // Not in the tag at all.
        assert!(!burns_in_daylight("minecraft:husk"));
        assert!(!burns_in_daylight("minecraft:zombified_piglin"));
        assert!(!burns_in_daylight("minecraft:skeleton_horse"));
        assert!(!burns_in_daylight("minecraft:creeper"));
        // In the tag, and fire-immune.
        assert!(!burns_in_daylight("minecraft:wither_skeleton"));
        // An un-namespaced id resolves like every other registry lookup.
        assert!(burns_in_daylight("zombie"));
    }

    /// A helmet answers every burner but one.
    #[test]
    fn only_the_phantom_shrugs_off_a_helmet() {
        assert!(helmet_helps("minecraft:zombie"));
        assert!(helmet_helps("minecraft:bogged"));
        assert!(!helmet_helps("minecraft:phantom"));
    }

    /// The burning hours, and the two that are not.
    #[test]
    fn only_clear_daytime_burns() {
        assert!(state_burns(WorldTime::Noon, WorldWeather::Clear));
        assert!(state_burns(WorldTime::Day, WorldWeather::Clear));
        assert!(!state_burns(WorldTime::Dusk, WorldWeather::Clear));
        assert!(!state_burns(WorldTime::Dawn, WorldWeather::Clear));
        assert!(!state_burns(WorldTime::Night, WorldWeather::Clear));
        assert!(!state_burns(WorldTime::Midnight, WorldWeather::Clear));
        // Rain and thunder gate `isSunBurnTick` off entirely.
        assert!(!state_burns(WorldTime::Noon, WorldWeather::Rain));
        assert!(!state_burns(WorldTime::Noon, WorldWeather::Thunder));
    }

    /// The radius is the declared `follow_range` or the one documented default —
    /// never a per-species table.
    #[test]
    fn the_radius_is_declared_or_the_one_default() {
        assert_eq!(stack_radius(None), DEFAULT_FOLLOW_RANGE);
        assert_eq!(
            stack_radius(Some(delvewright_dsl::MobAttributes {
                max_health: None,
                attack_damage: None,
                movement_speed: None,
                follow_range: Some(24.0),
            })),
            24
        );
    }
}
