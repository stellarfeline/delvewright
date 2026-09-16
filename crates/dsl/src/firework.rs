//! A firework's surface, and **every game fact it is built on, in one file**
//! (spec-0068).
//!
//! # Why the constants are here and not spread through the emitter
//!
//! Each number below is read off a page of the Minecraft Wiki rather than
//! measured on a server, and a cited number that is copied to its three call
//! sites is three numbers a re-pin has to find. So the pages are named
//! ([`WIKI_PAGES`]), each constant says which one it came from, and the
//! emitter, the refusal and the skill page all read them from here: re-pinning
//! the firework against a later game is one diff in this file.
//!
//! The rules built on the facts — the fixed `LifeTime`, the roof column, the
//! five-block reach — are **authored**, and they live with the check that
//! states them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The wiki pages every constant in this module was read from (spec-0068
/// *Ground*). Named so a re-pin knows what to re-read, and asserted by
/// `crates/dsl/tests/v29_firework.rs` so the list cannot quietly shrink.
pub const WIKI_PAGES: [&str; 3] = [
    "Firework Rocket",
    "Data component format/fireworks",
    "Data component format/equippable",
];

/// The vanilla entity a firework effect summons.
pub const ROCKET_ENTITY: &str = "minecraft:firework_rocket";

/// The item id the summoned rocket's own item stack carries.
pub const ROCKET_ITEM: &str = "minecraft:firework_rocket";

/// The item component that holds the bursts [cited — *Data component
/// format/fireworks*].
pub const FIREWORKS_COMPONENT: &str = "minecraft:fireworks";

/// **The entity's item field** [cited — *Firework Rocket*, entity data]. The
/// summoned rocket reads its bursts from the stack under this key; a renamed
/// key would ship a rocket that flies and shows nothing, so the emitted spelling
/// is asserted against this constant by the emission test.
pub const ITEM_FIELD: &str = "FireworksItem";

/// The shortest flight the game crafts, and this verb's default.
pub const MIN_FLIGHT: u8 = 1;

/// The longest flight the game crafts [cited — *Firework Rocket*]. Durations
/// beyond it are a non-goal: the wiki states a burst height for one, two and
/// three, and for nothing else.
pub const MAX_FLIGHT: u8 = 3;

/// At least one burst: a rocket with none is a flare that glides along whatever
/// it meets and shows nothing [cited — *Firework Rocket*].
pub const MIN_EXPLOSIONS: usize = 1;

/// At most seven bursts — **the game's own crafting cap**, and the largest count
/// the page states a damage for [cited — *Firework Rocket*]. The component would
/// take 256; a display of more is a `sequence` of rockets, not one rocket that
/// could kill an unhurt player by itself.
pub const MAX_EXPLOSIONS: usize = 7;

/// Damage a one-star burst deals, in HP, to a body within [`BLAST_RADIUS`]
/// blocks and not behind a solid block [cited — *Firework Rocket*].
pub const DAMAGE_ONE_STAR_HP: u32 = 7;

/// Additional HP per star beyond the first [cited — *Firework Rocket*].
pub const DAMAGE_PER_EXTRA_STAR_HP: u32 = 2;

/// How far a burst reaches, in blocks [cited — *Firework Rocket*].
pub const BLAST_RADIUS: i32 = 5;

/// The worst burst this verb can write, in HP: [`MAX_EXPLOSIONS`] stars.
///
/// Under a full body's twenty, which is the whole reason [`MAX_EXPLOSIONS`] is
/// seven — no rocket this verb writes can kill an unhurt player by itself.
#[must_use]
pub fn worst_damage_hp() -> u32 {
    DAMAGE_ONE_STAR_HP + DAMAGE_PER_EXTRA_STAR_HP * (MAX_EXPLOSIONS as u32 - 1)
}

/// The fixed term of the game's randomised `LifeTime`
/// (`(flight + 1) × LIFETIME_STEP_TICKS + random(0..5) + random(0..6)` ticks)
/// [cited — *Firework Rocket*, entity data].
pub const LIFETIME_STEP_TICKS: u32 = 10;

/// **The `LifeTime` the emitter writes**: the floor of the game's randomised
/// range, `(flight + 1) × 10` ticks.
///
/// Authored, on the cited formula. Left unset the game rolls the two random
/// terms at launch, so one datapack would burst at two heights and the reach
/// proof would be about a number nobody chose. The floor is the conservative
/// side of that proof: the burst is the lowest the game would ever put it.
#[must_use]
pub fn lifetime_ticks(flight: u8) -> u32 {
    (u32::from(flight) + 1) * LIFETIME_STEP_TICKS
}

/// The burst height in blocks above the launch cell, per flight duration
/// 1, 2, 3 — the **floor** of each range the wiki states (8–20, 18–34, 32–52),
/// because [`lifetime_ticks`] fixes `LifeTime` at the range's floor [cited —
/// *Firework Rocket*].
pub const BURST_HEIGHTS: [i32; 3] = [8, 18, 32];

/// The burst height for a flight duration, clamped to the crafted range.
#[must_use]
pub fn burst_height(flight: u8) -> i32 {
    let i = flight.clamp(MIN_FLIGHT, MAX_FLIGHT) as usize - 1;
    BURST_HEIGHTS[i]
}

/// One of the game's five explosion shapes [cited — *Data component
/// format/fireworks*], spelled as the component spells it.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FireworkShape {
    /// A small ball — the plain firework star.
    SmallBall,
    /// A large ball.
    LargeBall,
    /// A star-shaped burst (a gold-nugget star).
    Star,
    /// A creeper-face burst (a creeper-head star).
    Creeper,
    /// A burst (a feather star).
    Burst,
}

impl FireworkShape {
    /// Every shape the game has, in the component's own order.
    pub const ALL: [FireworkShape; 5] = [
        FireworkShape::SmallBall,
        FireworkShape::LargeBall,
        FireworkShape::Star,
        FireworkShape::Creeper,
        FireworkShape::Burst,
    ];

    /// The token the `minecraft:fireworks` component's `shape` field carries —
    /// the same string the DSL writes.
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            FireworkShape::SmallBall => "small_ball",
            FireworkShape::LargeBall => "large_ball",
            FireworkShape::Star => "star",
            FireworkShape::Creeper => "creeper",
            FireworkShape::Burst => "burst",
        }
    }
}

/// One burst of a [`crate::Verb::Firework`] — one firework star.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FireworkExplosion {
    /// Which of the game's five shapes this burst takes.
    pub shape: FireworkShape,
    /// The burst's colours, one or more `#rrggbb` literals — the spelling
    /// [`crate::PotionContents::color`] uses, emitted as the packed integers the
    /// component reads. At least one: a star with no colour is not a star.
    #[schemars(length(min = 1), inner(pattern(r"^#[0-9a-fA-F]{6}$")))]
    pub colors: Vec<String>,
    /// Colours the burst fades to, same spelling. Empty = no fade.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(inner(pattern(r"^#[0-9a-fA-F]{6}$")))]
    pub fade_colors: Vec<String>,
    /// Whether the burst trails (a diamond star). Default false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub trail: bool,
    /// Whether the burst twinkles/crackles (a glowstone-dust star). Default
    /// false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub twinkle: bool,
}

/// serde `skip_serializing_if` helper: skip a `false` flag.
fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_worst_burst_cannot_kill_an_unhurt_player() {
        assert_eq!(worst_damage_hp(), 19);
        assert!(worst_damage_hp() < 20);
    }

    #[test]
    fn lifetime_is_the_floor_of_the_games_range() {
        assert_eq!(lifetime_ticks(1), 20);
        assert_eq!(lifetime_ticks(2), 30);
        assert_eq!(lifetime_ticks(3), 40);
    }

    #[test]
    fn a_height_per_crafted_flight() {
        assert_eq!(burst_height(1), 8);
        assert_eq!(burst_height(2), 18);
        assert_eq!(burst_height(3), 32);
    }
}
