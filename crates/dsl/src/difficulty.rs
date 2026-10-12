//! The world difficulty a campaign ships at, and the bodies it stages.
//!
//! A campaign may declare `world.difficulty` (`easy` / `normal` / `hard`;
//! `peaceful` is refused, `DW0468`). Absent, the engine **derives** it, and the
//! derivation never picks a difficulty that removes a body the campaign stages:
//! peaceful is the only difficulty that removes any body (see
//! [`crate::registry::peaceful_despawn`]), so the derivation is `easy` when the
//! campaign fields any wave or stages any body of a type peaceful discards, and
//! `peaceful` otherwise. One function, read by the emitter (`server.properties`),
//! the combat arithmetic and the `DW0469` advisory alike.

use crate::envelope::Campaign;
use crate::{BodyRef, Verb, WorldDifficulty};

/// One body a campaign can put into the world, with the entity type it ships
/// as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedBody {
    /// What declares it: `wave-mob`, `npc`, `actor`, `actor-twin` (the
    /// real-AI body an `unleash-actor` summons) or `volley-projectile`.
    pub what: &'static str,
    /// The declaring object's id (the wave, NPC or actor id; for a volley, the
    /// JSON pointer of the `volley` effect).
    pub id: String,
    /// The entity type the body ships as.
    pub entity: String,
}

/// Every body the campaign stages whose entity type the author chose, in a
/// fixed order: wave mobs, NPCs, actors (the puppet as it ships — a skinned one
/// is a mannequin — then the twin when the actor is unleashed), volley
/// projectiles.
pub fn staged_bodies(c: &Campaign) -> Vec<StagedBody> {
    let q = &c.quests.content;
    let mut out = Vec::new();
    for w in &q.waves {
        for m in &w.mobs {
            out.push(StagedBody {
                what: "wave-mob",
                id: w.id.as_str().to_string(),
                entity: m.entity.clone(),
            });
        }
    }
    for n in &c.npcs.content.npcs {
        out.push(StagedBody {
            what: "npc",
            id: n.id.as_str().to_string(),
            entity: BodyRef::Npc(n).worn_entity().to_string(),
        });
    }
    let unleashed = crate::fight::unleashed_actors(c);
    for a in &q.actors {
        out.push(StagedBody {
            what: "actor",
            id: a.id.as_str().to_string(),
            entity: BodyRef::Actor(a).worn_entity().to_string(),
        });
        if unleashed.contains(a.id.as_str()) {
            out.push(StagedBody {
                what: "actor-twin",
                id: a.id.as_str().to_string(),
                entity: a.entity.clone(),
            });
        }
    }
    crate::for_each_campaign_effect(c, &mut |path, _, eff| {
        if let Verb::Volley { projectile, .. } = &eff.verb {
            out.push(StagedBody {
                what: "volley-projectile",
                id: path.to_string(),
                entity: projectile
                    .as_deref()
                    .unwrap_or(crate::DEFAULT_VOLLEY_PROJECTILE)
                    .to_string(),
            });
        }
    });
    out
}

/// The staged bodies the pinned game would discard on peaceful.
pub fn bodies_removed_on_peaceful(c: &Campaign) -> Vec<StagedBody> {
    staged_bodies(c)
        .into_iter()
        .filter(|b| crate::registry::removed_on_peaceful(&b.entity))
        .collect()
}

/// The difficulty the engine derives for a campaign that declares none: the
/// lowest that keeps every staged body and every declared fight. `easy` when
/// the campaign fields any wave or stages a body peaceful discards, else
/// `peaceful`.
pub fn derived_difficulty(c: &Campaign) -> WorldDifficulty {
    if !c.quests.content.waves.is_empty() || !bodies_removed_on_peaceful(c).is_empty() {
        WorldDifficulty::Easy
    } else {
        WorldDifficulty::Peaceful
    }
}

/// The difficulty the delve ships at: the declared one, else
/// [`derived_difficulty`].
pub fn effective_difficulty(c: &Campaign) -> WorldDifficulty {
    c.world
        .content
        .difficulty
        .unwrap_or_else(|| derived_difficulty(c))
}
