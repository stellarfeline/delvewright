//! Engagement proof: a body staged as a fight the party must win is one whose own
//! vanilla AI will engage the party under the delve's declared hour, weather and
//! footing (`DW0920`).
//!
//! The mirror of `DW0496` ([`crate::compiler::daylight`]): that rule refuses a
//! fight the sun settles for the party; this one refuses a fight the body walks
//! away from.
//!
//! ## The defect this exists for (`vesperhold`)
//!
//! The Drowned Choir is an elite wave of four `minecraft:drowned` in the Undertide
//! Pool, adjudicated by a `kill` objective. The delve is pinned to `dusk` in
//! `rain`. In play the choir did not attack: every body walked toward the well's
//! water, the curb that keeps them from drowning in it kept them out, and the
//! party cut them down while they stood facing the wall. Every rung was green —
//! the wave had footing, the room was reachable, the fight was winnable by
//! arithmetic — and none of them asks whether the body fights at all.
//!
//! ## Why a drowned does not fight there — read from the pinned jar
//!
//! `Drowned.okTarget(target)` is `!level.isBrightOutside() || target.isInWater()`.
//! It is the predicate on the drowned's player-targeting goal and the gate on its
//! melee goal (`Drowned$DrownedAttackGoal.canUse` / `canContinueToUse`), so while
//! the level is bright a drowned neither acquires nor strikes a target that is not
//! in water, and `Drowned$DrownedGoToWaterGoal` (priority 1, `canUse` gated on the
//! same `isBrightOutside`) walks it toward water instead. That is the report,
//! exactly.
//!
//! `Level.isBrightOutside()` is `!dimensionType.hasFixedTime() && skyDarken < 4`,
//! and in 1.21.11 `skyDarken` is `(int)(15 - gameplay/sky_light_level)`. The
//! overworld's `minecraft:day` timeline multiplies the base 15 by a factor held at
//! 1.0 from tick 133 to 11867 and falling to 0.26666668 at 13670 (linear); rain
//! alpha-blends the level toward 4.0 at 0.3125 and thunder at 0.52734375
//! (`WeatherAttributes`). At the six declared hours that gives:
//!
//! | hour (tick) | clear | rain | thunder |
//! |---|---|---|---|
//! | day (1000), noon (6000) | 15.00 → bright | 11.56 → bright | 9.20 → dark |
//! | dusk (12000) | 14.19 → bright | 11.005 → bright | 8.82 → dark |
//! | night (13000), dawn (23000) | 8.09 → dark | | |
//! | midnight (18000) | 4.00 → dark | | |
//!
//! [`bright_outside`] is that table. `dusk` in `rain` is the tight cell: `skyDarken`
//! is `(int)3.995 = 3`, bright by five thousandths of a light level, which is why the
//! table was also taken from the running game rather than from the arithmetic
//! alone: `tools/maintenance/probe-drowned-engagement.py` summons a drowned beside a
//! villager in a roofed cell under each of the eighteen states on the pinned server
//! and reads whether it was struck. Both methods agree on all eighteen.
//!
//! ### Which bodies — one, and why it is written here
//!
//! No vanilla tag names the bodies whose targeting reads the hour. The pinned jar
//! was scanned for every class that calls `Level.isBrightOutside()`: thirteen do,
//! and the only one that gates a *target* is `Drowned` (the others are the fox, the
//! wandering trader, the drowned's own goals, and the sun-avoiding and village-
//! strolling goals). So [`DISENGAGES_ON_LAND_WHILE_BRIGHT`] is one entity id with
//! that measurement as its citation. A spider's neutrality reads the light at its
//! own position, not the declared hour, and is not this rule's.
//!
//! ## The rule
//!
//! A staged combatant is `DW0920` when all four hold:
//!
//! 1. **Its AI refuses a land target while bright.** The entity is a drowned.
//! 2. **It is a fight.** A `kill` objective adjudicates its wave, or it is an actor
//!    the party can damage — the population `DW0496` reads
//!    ([`crate::compiler::daylight::collect_staged`]), one definition of "a fight".
//! 3. **The level is bright for as long as the fight is live.** Every state the
//!    body can stand in from the beat that puts it in the world until the wave's
//!    `kill` objective completes is [`bright_outside`] — read off `DW0496`'s
//!    [`Clock`], the one model of which cuts a body can meet, with the span ended
//!    at the `kill` whether or not a rest re-seats the wave (a body met again
//!    after the party has won is no longer a fight it must win). An actor, which
//!    no objective dates, runs to the end of the delve. Vesperhold cuts to noon,
//!    night and dawn for its echoes and endings, every one of them in quests
//!    after the choir, so the choir's span holds dusk in rain alone.
//! 4. **The party cannot put its feet in water within its reach.** No cell a body
//!    can stand in with its feet in water lies within the stack's aggro radius, on
//!    ground walk-reachable from where the stack is seated ([`wet_footing_within_reach`]).
//!    Two shapes count as wet: a standable cell whose support is a waterlogged
//!    block whose top is below the water's surface (a waterlogged bottom slab — the
//!    feet stand in its water, measured on the pinned server), and a flooded cell a
//!    body can walk or drop into from a reachable standable neighbour. The radius
//!    and the walk are `DW0496`'s: the declared `follow_range` or
//!    [`crate::compiler::nav::DEFAULT_FOLLOW_RANGE`], and an unbounded walk from the
//!    seated cells. Any wet cell silences the rule, so it under-fires by
//!    construction: one puddle in reach is enough for it to hold its tongue.
//!
//! ## Prescription
//!
//! Put the party's feet in water where the fight happens (a floor of waterlogged
//! bottom slabs is standable ground the model already walks), stage a species
//! that engages on land, or declare a dark hour or thunder. Never pull the body
//! out of reach of its own water by walling it in: that is what turned the choir
//! toward the wall.

use std::collections::BTreeMap;

use delvewright_dsl::{DwCode, ExitTier, WorldTime, WorldWeather};

use crate::compiler::daylight::{Clock, Sky, Staged, collect_staged, kill_objectives};
use crate::compiler::failure::Failure;
use crate::compiler::nav::World;
use crate::compiler::plan::Plan;

/// `DW0920`: a body whose vanilla AI will not engage a land target under the
/// delve's bright hour is staged as a fight the party must win, with no water in
/// reach for the party to stand in.
pub const DW_WILL_NOT_ENGAGE: DwCode = DwCode::new("DW0920", ExitTier::Build);

/// The bodies whose targeting reads the declared hour: `Drowned.okTarget` is the
/// one target predicate in the pinned 1.21.11 jar that calls
/// `Level.isBrightOutside()` (thirteen classes call it; this is the only target
/// gate among them — see the module docs).
const DISENGAGES_ON_LAND_WHILE_BRIGHT: [&str; 1] = ["minecraft:drowned"];

/// A water source's surface above its cell floor, in sixteenths: vanilla's
/// `FluidState.getHeight` for a source is `8 / 9` of a block. A body whose feet
/// stand below it, in the cell, is in the water.
const SOURCE_SURFACE_16: f64 = 16.0 * 8.0 / 9.0;

/// How far a body stepping off an edge is followed down in search of water.
const MAX_DROP: i32 = 64;

/// Whether `Level.isBrightOutside()` holds at this frozen `(time, weather)` on the
/// pinned server — the table in the module docs, measured two ways.
pub fn bright_outside(time: WorldTime, weather: WorldWeather) -> bool {
    matches!(time, WorldTime::Day | WorldTime::Noon | WorldTime::Dusk)
        && matches!(weather, WorldWeather::Clear | WorldWeather::Rain)
}

/// The states in effect while the fight is live, when every one of them is
/// bright; `None` when any is dark.
///
/// Read off `DW0496`'s [`Clock`]: from the beat that puts the body in the world
/// until the fight is decided. For a wave, that is its `kill` objectives —
/// whether or not a rest re-seats it, since a body met again after the party has
/// won is no longer a fight the party must win. An actor, which no objective
/// dates, runs to the end of the delve.
fn bright_while_live(c: &delvewright_dsl::Campaign, clock: &Clock, body: &Staged) -> Option<Sky> {
    let until = if body.kind == "wave" {
        kill_objectives(c, &body.owner)
    } else {
        body.dead_after.clone()
    };
    let sky = clock.sky_for(c, &body.beats, &until);
    sky.times
        .iter()
        .all(|&t| sky.weathers.iter().all(|&w| bright_outside(t, w)))
        .then_some(sky)
}

fn disengages_on_land(entity: &str) -> bool {
    DISENGAGES_ON_LAND_WHILE_BRIGHT
        .contains(&crate::compiler::registry::namespaced_entity(entity).as_str())
}

/// Whether a body standing in `cell` has its feet in the water of the
/// waterlogged block it stands on.
fn stands_in_water(world: &World, blocks: &BTreeMap<[i32; 3], String>, cell: [i32; 3]) -> bool {
    let support = [cell[0], cell[1] - 1, cell[2]];
    let Some(name) = blocks.get(&support) else {
        return false;
    };
    if !crate::compiler::assembled::is_waterlogged(name) {
        return false;
    }
    let feet_16 = world.feet_y(cell) * 16.0 - f64::from(support[1]) * 16.0;
    feet_16 < SOURCE_SURFACE_16
}

/// The first flooded cell a body can walk or drop into from standable `from`
/// through its cardinal neighbour column `(x, z)`, if any.
fn water_off_the_edge(world: &World, from: [i32; 3], x: i32, z: i32) -> Option<[i32; 3]> {
    let head = [x, from[1] + 1, z];
    if world.solid_top_16(head).is_some() || world.is_barrier(head) {
        return None;
    }
    for depth in 0..=MAX_DROP {
        let c = [x, from[1] - depth, z];
        if world.is_flooded(c) {
            return Some(c);
        }
        if !world.is_clear(c) || world.is_barrier(c) {
            return None;
        }
    }
    None
}

/// The nearest cell within `radius` of `from` where a body walking from `from`
/// can have its feet in water. Deterministic (ADR-0006): the reachable set is a
/// `BTreeSet` and ties break on `(d², cell)`.
fn wet_footing_within_reach(
    world: &World,
    blocks: &BTreeMap<[i32; 3], String>,
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
    let mut best: Option<(i64, [i32; 3])> = None;
    let mut consider = |cell: [i32; 3]| {
        let d = d2(cell);
        if d <= r2 && best.is_none_or(|b| (d, cell) < b) {
            best = Some((d, cell));
        }
    };
    for cell in world.reachable_walkable(from) {
        if stands_in_water(world, blocks, cell) {
            consider(cell);
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if let Some(w) = water_off_the_edge(world, cell, cell[0] + dx, cell[2] + dz) {
                consider(w);
            }
        }
    }
    best.map(|(_, c)| c)
}

/// What the engagement proof examined, printed on every build that seats waves,
/// zeroes included.
#[derive(Debug, Default)]
pub struct EngageBinding {
    /// Staged fights examined: wave stacks a `kill` objective adjudicates and
    /// actors the party can damage.
    pub fights: usize,
    /// Of those, the ones whose species' targeting reads the hour.
    pub hour_reading: usize,
    /// Of those, the ones bright for as long as the fight is live.
    pub bright: usize,
    /// Of those, the ones with water for the party's feet within reach.
    pub wet: usize,
    /// Refused (`DW0920`): bright, and dry within reach.
    pub refused: usize,
}

impl EngageBinding {
    /// The one line this proof owes its reader.
    pub fn line(&self) -> String {
        format!(
            "engagement binding: {} staged fight(s) examined, {} of a species whose targeting \
             reads the hour, {} of those bright while the fight is live, {} of those with water \
             for the party's feet within reach; {} refused (DW0920).",
            self.fights, self.hour_reading, self.bright, self.wet, self.refused
        )
    }
}

/// Prove no staged fight is one its bodies' own AI will not take up (`DW0920`).
///
/// `spawns` is the seated wave placement (`emit::plan_wave_spawns`), the cells the
/// datapack summons on — the same input `DW0496` measures from. Every body is
/// examined before the first refusal is returned, so the binding counts the
/// whole population either way.
pub fn check_engagement(
    plan: &Plan,
    world: &World,
    blocks: &BTreeMap<[i32; 3], String>,
    spawns: &BTreeMap<String, Vec<[i32; 3]>>,
) -> (EngageBinding, Option<Failure>) {
    let c = plan.campaign;
    let clock = Clock::new(c);
    let mut b = EngageBinding::default();
    let mut first: Option<Failure> = None;
    for body in collect_staged(plan, spawns, &clock) {
        b.fights += 1;
        if !disengages_on_land(&body.entity) {
            continue;
        }
        b.hour_reading += 1;
        let Some(states) = bright_while_live(c, &clock, &body) else {
            continue;
        };
        b.bright += 1;
        if wet_footing_within_reach(world, blocks, &body.cells, body.radius).is_some() {
            b.wet += 1;
            continue;
        }
        b.refused += 1;
        first.get_or_insert_with(|| Failure {
            code: DW_WILL_NOT_ENGAGE,
            message: message(&body, &states),
        });
    }
    (b, first)
}

fn message(body: &Staged, Sky { times, weathers }: &Sky) -> String {
    let Staged {
        owner,
        kind,
        entity,
        cells,
        radius,
        ..
    } = body;
    let at = cells[0];
    let list = |words: Vec<&str>| words.join("`, `");
    let time = list(times.iter().map(|t| t.keyword()).collect());
    let weather = list(weathers.iter().map(|w| w.keyword()).collect());
    format!(
        "{kind} `{owner}` stages `{entity}` at [{}, {}, {}] as a fight the party must win, and \
         vanilla's own AI will not fight it there: a drowned neither targets nor strikes anyone \
         who is not in water while the level is bright (`Drowned.okTarget`), and walks toward \
         water instead. The level is bright for as long as this fight is live (the hour can be \
         `{time}` and the weather `{weather}` before its `kill` objective completes; only \
         thunder, `night`, `midnight` or `dawn` is dark to it), and no cell \
         within this stack's {radius}-block aggro radius, on ground walkable from where it is \
         seated, puts a body's feet in water. This is the Drowned Choir in vesperhold, which \
         walked at its well's curb while the party cut it down. Fix the content: flood the \
         floor the fight happens on (a floor of waterlogged bottom slabs stands a body in \
         water and is ground the route proofs walk), or stage a species that fights on land. \
         Do NOT wall the body away from its water to keep it alive — that is what turns it \
         toward the wall.",
        at[0], at[1], at[2],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The eighteen states, as the pinned jar and the pinned server both answer.
    #[test]
    fn bright_is_day_noon_or_dusk_unless_thunder() {
        for t in [WorldTime::Day, WorldTime::Noon, WorldTime::Dusk] {
            assert!(bright_outside(t, WorldWeather::Clear));
            assert!(bright_outside(t, WorldWeather::Rain));
            assert!(!bright_outside(t, WorldWeather::Thunder));
        }
        for t in [WorldTime::Night, WorldTime::Midnight, WorldTime::Dawn] {
            for w in [
                WorldWeather::Clear,
                WorldWeather::Rain,
                WorldWeather::Thunder,
            ] {
                assert!(!bright_outside(t, w));
            }
        }
    }

    /// A 9x9 stone floor at y=0, walked from its centre, with `edit` applied to
    /// the occupancy and the block map.
    fn yard(
        edit: impl FnOnce(&mut crate::compiler::assembled::Occupancy, &mut BTreeMap<[i32; 3], String>),
    ) -> (World, BTreeMap<[i32; 3], String>) {
        let mut occ = crate::compiler::assembled::Occupancy {
            solid: std::collections::BTreeSet::new(),
            tall: std::collections::BTreeSet::new(),
            use_gates: std::collections::BTreeSet::new(),
            flooded: std::collections::BTreeSet::new(),
            partial: BTreeMap::new(),
            waterloggable: std::collections::BTreeSet::new(),
        };
        let mut blocks = BTreeMap::new();
        for x in 0..9 {
            for z in 0..9 {
                occ.solid.insert([x, 0, z]);
                blocks.insert([x, 0, z], "minecraft:stone".to_string());
            }
        }
        edit(&mut occ, &mut blocks);
        (
            World::from_occupancy(occ, crate::compiler::nav::Premises::geometry_only()),
            blocks,
        )
    }

    const FROM: [[i32; 3]; 1] = [[4, 1, 4]];

    #[test]
    fn dry_stone_is_dry() {
        let (w, b) = yard(|_, _| {});
        assert_eq!(wet_footing_within_reach(&w, &b, &FROM, 16), None);
    }

    /// A waterlogged bottom slab stands the feet in its water; the same slab dry,
    /// or a waterlogged TOP slab (feet on its top face, above the water), does not.
    #[test]
    fn a_waterlogged_bottom_slab_is_wet_and_nothing_else_about_it_is() {
        let slab = |state: &'static str, partial: Option<u8>| {
            move |occ: &mut crate::compiler::assembled::Occupancy,
                  blocks: &mut BTreeMap<[i32; 3], String>| {
                blocks.insert([7, 0, 4], state.to_string());
                if let Some(h) = partial {
                    occ.partial.insert([7, 0, 4], h);
                }
            }
        };
        let (w, b) = yard(slab(
            "minecraft:tuff_slab[type=bottom,waterlogged=true]",
            Some(8),
        ));
        assert_eq!(wet_footing_within_reach(&w, &b, &FROM, 16), Some([7, 1, 4]));
        // Out of perception: the same cell beyond the radius.
        assert_eq!(wet_footing_within_reach(&w, &b, &FROM, 2), None);
        let (w, b) = yard(slab(
            "minecraft:tuff_slab[type=bottom,waterlogged=false]",
            Some(8),
        ));
        assert_eq!(wet_footing_within_reach(&w, &b, &FROM, 16), None);
        let (w, b) = yard(slab("minecraft:tuff_slab[type=top,waterlogged=true]", None));
        assert_eq!(wet_footing_within_reach(&w, &b, &FROM, 16), None);
    }

    /// Open water a body can step or drop into from walkable ground counts; the
    /// same water behind a wall does not.
    #[test]
    fn water_off_the_edge_is_wet_unless_walled() {
        let pit = |occ: &mut crate::compiler::assembled::Occupancy,
                   _: &mut BTreeMap<[i32; 3], String>| {
            occ.solid.remove(&[4, 0, 8]);
            occ.solid.insert([4, -3, 8]);
            occ.flooded.insert([4, -2, 8]);
        };
        let (w, b) = yard(pit);
        assert_eq!(
            wet_footing_within_reach(&w, &b, &FROM, 16),
            Some([4, -2, 8])
        );
        let (w, b) = yard(|occ, blocks| {
            pit(occ, blocks);
            occ.tall.insert([4, 1, 8]);
        });
        assert_eq!(wet_footing_within_reach(&w, &b, &FROM, 16), None);
    }

    /// The live-game instrument holds the same table this module does: its
    /// `DISENGAGED` set is exactly the bright states, so a probe run that agrees
    /// with itself agrees with the compiler.
    #[test]
    fn the_probe_measures_this_table() {
        let probe = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/maintenance/probe-drowned-engagement.py"
        ))
        .unwrap();
        let line = probe
            .lines()
            .find(|l| l.starts_with("DISENGAGED = "))
            .expect("the probe states its table");
        let times = [
            WorldTime::Day,
            WorldTime::Noon,
            WorldTime::Dusk,
            WorldTime::Night,
            WorldTime::Midnight,
            WorldTime::Dawn,
        ];
        let weathers = [
            WorldWeather::Clear,
            WorldWeather::Rain,
            WorldWeather::Thunder,
        ];
        let bright_times: Vec<&str> = times
            .iter()
            .filter(|&&t| weathers.iter().any(|&w| bright_outside(t, w)))
            .map(|t| t.keyword())
            .collect();
        let bright_weathers: Vec<&str> = weathers
            .iter()
            .filter(|&&w| times.iter().any(|&t| bright_outside(t, w)))
            .map(|w| w.keyword())
            .collect();
        // The table is a product, so it is stated as one.
        for &t in &times {
            for &w in &weathers {
                assert_eq!(
                    bright_outside(t, w),
                    bright_times.contains(&t.keyword()) && bright_weathers.contains(&w.keyword())
                );
            }
        }
        let quoted = |v: &[&str]| {
            v.iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        assert_eq!(
            line,
            format!(
                "DISENGAGED = {{(t, w) for t in ({}) for w in ({})}}",
                quoted(&bright_times),
                quoted(&bright_weathers)
            )
        );
    }

    #[test]
    fn only_the_drowned_reads_the_hour_for_its_target() {
        assert!(disengages_on_land("minecraft:drowned"));
        assert!(disengages_on_land("drowned"));
        assert!(!disengages_on_land("minecraft:zombie"));
        assert!(!disengages_on_land("minecraft:husk"));
    }
}
