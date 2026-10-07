//! A lightning strike's surface, and **every game fact it is built on, in one
//! file** (spec-0092).
//!
//! Each number below is read from the pinned 1.21.11 game's own bytes —
//! `net.minecraft.world.entity.LightningBolt` and `Entity.thunderHit`,
//! disassembled with `javap` and named through Mojang's official 1.21.11
//! mappings — and the dedicated server's class is byte-identical to the
//! client's, so the facts are the server's. The emitter, the refusals and the
//! skill page read them from here: re-pinning the strike against a later game
//! is one diff in this file.
//!
//! The rules built on the facts — the reach box judged on cells, the refused
//! blocks — are **authored**, and they live with the check that states them
//! (`delvec`'s `compiler::lightning`).

/// The classes the facts below were read from (spec-0092 *Ground*). Named so a
/// re-pin knows what to re-read, and asserted by the test that lands the verb.
pub const READ_FROM: [&str; 4] = [
    "net.minecraft.world.entity.LightningBolt",
    "net.minecraft.world.entity.Entity#thunderHit",
    "net.minecraft.server.level.ServerLevel#canSpreadFireAround",
    "net.minecraft.server.level.ChunkMap#anyPlayerCloseEnoughTo",
];

/// The vanilla entity a `lightning` effect summons.
pub const BOLT_ENTITY: &str = "minecraft:lightning_bolt";

/// How far the bolt reaches sideways, in blocks, on each horizontal axis
/// [cited — `LightningBolt.tick`, the `3.0` it inflates its position by].
pub const REACH_HORIZONTAL: i32 = 3;

/// How far the bolt reaches below its position, in blocks [cited — the same
/// `3.0`].
pub const REACH_BELOW: i32 = 3;

/// How far the bolt reaches above its position, in blocks [cited — `6.0 + 3.0`].
pub const REACH_ABOVE: i32 = 9;

/// Damage the bolt deals a body in reach, in HP [cited — `Entity.thunderHit`,
/// `hurtServer(…, lightningBolt(), 5.0F)`].
pub const DAMAGE_HP: u32 = 5;

/// Seconds a body in reach burns for [cited — `Entity.thunderHit`,
/// `igniteForSeconds(8.0F)`], at one HP a second while it burns.
pub const BURN_SECONDS: u32 = 8;

/// The worst a strike does to an unarmoured player in reach, in HP: the hit and
/// the whole burn. Under a full body's twenty.
#[must_use]
pub fn worst_damage_hp() -> u32 {
    DAMAGE_HP + BURN_SECONDS
}

/// The gamerule whose value decides whether a bolt lights fire [cited —
/// `LightningBolt.spawnFire` asks `ServerLevel.canSpreadFireAround`, which asks
/// whether a non-spectator player stands strictly closer than this radius].
/// Every delve seals it at `0`, and no distance is less than zero.
pub const FIRE_GAMERULE: &str = "fire_spread_radius_around_player";

/// **Whether the bolt rewrites the block it strikes** [cited —
/// `LightningBolt.powerLightningRod` and `clearCopperOnLightningStrike`]: a
/// lightning rod is powered, and a weathering or waxed copper block is scraped
/// back along a random walk through the copper beside it — a world write the
/// game rolls.
///
/// `block` is a block id with or without its state suffix. The copper family is
/// read by name: every weathering and waxed copper block of the pinned game
/// carries `copper` in its id. Copper ore and raw copper match the name too and
/// are refused with the family, which errs in the safe direction.
#[must_use]
pub fn rewrites(block: &str) -> bool {
    let id = block.split('[').next().unwrap_or(block);
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    id == "lightning_rod" || id.ends_with("_lightning_rod") || id.contains("copper")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rod_and_the_copper_family_are_rewritten() {
        for b in [
            "minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]",
            "minecraft:copper_block",
            "minecraft:waxed_exposed_cut_copper_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]",
            "oxidized_copper",
        ] {
            assert!(rewrites(b), "{b}");
        }
        for b in [
            "minecraft:stone",
            "minecraft:water[level=0]",
            "minecraft:dark_prismarine",
        ] {
            assert!(!rewrites(b), "{b}");
        }
    }

    #[test]
    fn the_worst_strike_is_under_a_full_body() {
        assert_eq!(worst_damage_hp(), 13);
        assert!(worst_damage_hp() < 20);
    }
}
