//! **How far an emitted mob keeps a target** — the one reading of a fight's
//! reach that every proof over a hostile's range takes.
//!
//! A mob keeps a target only while it stands within its `minecraft:follow_range`
//! (`TargetGoal.canContinueToUse`, `docs/reference/lane-engagement.md` §1), so
//! the reach of a fight is the follow range the emitted body actually carries
//! once it fights:
//!
//! - a **lane** mob marches with its follow range held at the lane's
//!   `aggro_radius`, and the clock that releases it resets the base to its
//!   species' own value, never narrower than the radius (`emit/lane.rs`,
//!   spec-0016 §6) — so once engaged it reaches the wider of the two;
//! - any other mob carries its declared `follow_range`, else its species'
//!   value;
//! - an actor likewise, from the body it wears.
//!
//! A fight not yet begun is noticed at a different distance: a marching lane
//! mob takes up a fight only within its radius ([`mob_perception`]). A proof
//! about a fight under way (where its bodies chase, fall, burn, stand in
//! water) reads the reach; a proof about a body the party meets fresh (a
//! respawn point, a way around, a fight found) reads the perception.
//!
//! The species' value is read from the pinned jar
//! (`crates/delvec/data/entity-engagement-1.21.11.json`, written by
//! `tools/maintenance/extract-entity-engagement.py`): pillager 32, vindicator
//! 12, zombie 35, skeleton 16. A kind the table gives no follow range (no
//! living body) keeps no target and reaches nothing.
//!
//! The same table names each kind's mob category; a stack of a `monster` kind
//! is a hostile the engagement PackTest binds (`emit/packtest/engage.rs`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

/// One kind's row of the pinned table.
#[derive(Clone, Debug, PartialEq)]
pub struct Species {
    /// The game's mob category (`monster`, `creature`, `misc`, …).
    pub category: String,
    /// The base value of `minecraft:follow_range`, when the kind carries it.
    pub follow_range: Option<f64>,
}

/// The pinned table, by namespaced id.
fn table() -> &'static BTreeMap<String, Species> {
    static TABLE: OnceLock<BTreeMap<String, Species>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let raw: BTreeMap<String, Value> =
            serde_json::from_str(include_str!("../../../data/entity-engagement-1.21.11.json"))
                .expect("the vendored entity-engagement table parses");
        raw.into_iter()
            .map(|(k, v)| {
                (
                    k,
                    Species {
                        category: v["category"].as_str().unwrap_or("").to_string(),
                        follow_range: v["follow_range"].as_f64(),
                    },
                )
            })
            .collect()
    })
}

/// The pinned row of `entity` (bare or namespaced).
pub fn species(entity: &str) -> Option<&'static Species> {
    let id = if entity.contains(':') {
        entity.to_string()
    } else {
        format!("minecraft:{entity}")
    };
    table().get(&id)
}

/// The species' own follow range — what `attribute … base reset` restores.
pub fn species_follow_range(entity: &str) -> Option<f64> {
    species(entity).and_then(|s| s.follow_range)
}

/// Whether the game files `entity` under the `monster` category: a hostile.
pub fn is_monster(entity: &str) -> bool {
    species(entity).is_some_and(|s| s.category == "monster")
}

/// The species' follow range, or 0 for a kind that keeps no target.
fn species_reach(entity: &str) -> f64 {
    species_follow_range(entity).unwrap_or(0.0)
}

/// **The reach one emitted mob of `w` has once it fights**, with the words a
/// message names it by.
pub fn mob_reach(w: &delvewright_dsl::Wave, mob: &delvewright_dsl::WaveMob) -> (f64, &'static str) {
    match &w.lane {
        Some(l) => {
            let r = f64::from(l.aggro_radius);
            let s = species_reach(&mob.entity);
            if s > r {
                (
                    s,
                    "the species' `follow_range`, which a released lane mob is reset to",
                )
            } else {
                (
                    r,
                    "the lane's `aggro_radius`, the floor a released lane mob keeps",
                )
            }
        }
        None => match mob.attributes.and_then(|a| a.follow_range) {
            Some(r) => (r, "the stack's declared `follow_range`"),
            None => (
                species_reach(&mob.entity),
                "the species' `follow_range` (none declared)",
            ),
        },
    }
}

/// **The distance at which one emitted mob of `w` takes up a fight** it is
/// not yet in: a lane mob marches with its follow range held at the lane's
/// `aggro_radius` and is released only by a player inside it, a target it
/// holds or a player who hurt it, so it notices at the radius; any other mob
/// at its [`mob_reach`]. The reading of a proof about a body the party meets
/// fresh — a respawn point (`DW0478`), a way around an optional fight
/// (`DW0380`), a fight found by reach (`DW0863`), where a lapse waits.
pub fn mob_perception(
    w: &delvewright_dsl::Wave,
    mob: &delvewright_dsl::WaveMob,
) -> (f64, &'static str) {
    match &w.lane {
        Some(l) => (
            f64::from(l.aggro_radius),
            "the lane's `aggro_radius`, at which a marching lane mob notices",
        ),
        None => mob_reach(w, mob),
    }
}

/// **A wave's perception**: the widest [`mob_perception`] of its stacks — a
/// fight not yet begun that the party comes within this distance of finds
/// the party.
pub fn wave_perception(w: &delvewright_dsl::Wave) -> (f64, &'static str) {
    widest(w.mobs.iter().map(|m| mob_perception(w, m)))
}

fn widest(it: impl Iterator<Item = (f64, &'static str)>) -> (f64, &'static str) {
    it.fold(None::<(f64, &'static str)>, |acc, r| match acc {
        Some(a) if a.0 >= r.0 => Some(a),
        _ => Some(r),
    })
    .unwrap_or((0.0, "no mob"))
}

/// **A wave's reach**: the widest [`mob_reach`] of its stacks — how far a
/// fight under way carries its bodies after the party.
pub fn wave_reach(w: &delvewright_dsl::Wave) -> (f64, &'static str) {
    widest(w.mobs.iter().map(|m| mob_reach(w, m)))
}

/// **An actor's reach**: its declared `follow_range`, else the species' value
/// of the body it wears.
pub fn actor_reach(a: &delvewright_dsl::Actor) -> (f64, &'static str) {
    match a.attributes.and_then(|at| at.follow_range) {
        Some(r) => (r, "the actor's declared `follow_range`"),
        None => (
            species_reach(&super::actor_body_entity(a)),
            "the species' `follow_range` (none declared)",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pinned jar's values the lane reference cites, read through the
    /// table (`docs/reference/lane-engagement.md` §1).
    #[test]
    fn the_table_reads_the_jars_follow_ranges() {
        assert_eq!(species_follow_range("minecraft:pillager"), Some(32.0));
        assert_eq!(species_follow_range("minecraft:vindicator"), Some(12.0));
        assert_eq!(species_follow_range("zombie"), Some(35.0));
        assert_eq!(species_follow_range("minecraft:player"), None);
        assert!(is_monster("minecraft:pillager"));
        assert!(!is_monster("minecraft:goat"));
    }

    fn wave(lane: Option<u32>, entity: &str, declared: Option<f64>) -> delvewright_dsl::Wave {
        let mut v = serde_json::json!({
            "id": "wave/w", "anchor": "anchor/a",
            "mobs": [{ "entity": entity, "count": 1 }]
        });
        if let Some(r) = declared {
            v["mobs"][0]["attributes"] = serde_json::json!({ "follow_range": r });
        }
        if let Some(r) = lane {
            v["lane"] = serde_json::json!({ "aggro_radius": r, "waypoints": ["anchor/b"] });
        }
        serde_json::from_value(v).expect("a wave")
    }

    /// A released lane mob chases to its species' range, never narrower than
    /// the lane's radius; the radius alone was the reading before.
    #[test]
    fn a_lane_mob_reaches_its_species_range_once_released() {
        let w = wave(Some(8), "minecraft:pillager", None);
        assert_eq!(mob_reach(&w, &w.mobs[0]).0, 32.0);
        let w = wave(Some(14), "minecraft:vindicator", None);
        assert_eq!(mob_reach(&w, &w.mobs[0]).0, 14.0);
        let w = wave(None, "minecraft:pillager", None);
        assert_eq!(wave_reach(&w).0, 32.0);
        let w = wave(None, "minecraft:pillager", Some(10.0));
        assert_eq!(wave_reach(&w).0, 10.0);
        // A marching lane mob notices at its radius; a plain one at its reach.
        let w = wave(Some(8), "minecraft:pillager", None);
        assert_eq!((wave_perception(&w).0, wave_reach(&w).0), (8.0, 32.0));
        let w = wave(None, "minecraft:spider", None);
        assert_eq!(wave_perception(&w).0, 16.0);
    }
}
