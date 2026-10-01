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
//! 3. **The sun can be up while it fights.** Some state the delve can be in
//!    between the body's entering and its death runs the burn tick at a
//!    sky-open cell — the pinned hour window, and no rain falling there
//!    ([`Clock`], [`hour_burns`], [`precipitates_at`]).
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
//! (`DW0475`) and aggro range ([`crate::compiler::nav::DEFAULT_FOLLOW_RANGE`]).
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
//!   [`crate::compiler::nav::DEFAULT_FOLLOW_RANGE`]: one documented number, never a
//!   per-species table, exactly as `DW0380`'s optional-elite spheres and
//!   `DW0478`'s bonfire aggro test read it.
//! * **Walkable** — [`crate::compiler::nav::World::reachable_walkable`] from the seated
//!   spawn cells over the assembled (and stage-7 edited) world, unbounded: getting
//!   there is a question about geometry, not about perception. Bounding the *walk*
//!   by the radius would have been green on the incident — the yard is 15.6 blocks
//!   from the muster room but 21 steps of corridor away.
//! * **Open sky** — [`crate::compiler::light::LightModel::sky_open`], the same geometric
//!   column test spec-0010's relight seeds sky light with. One model of "the sky is
//!   above this cell" in the compiler, not two.
//!
//! ### 3. Which hours and weathers burn, and when a fight stands in them
//!
//! **The hour is the pinned game's, never ours.** 1.21.11 moved the burn gate
//! off `Level.isDay()` onto an environment attribute: `Mob.isSunBurnTick`
//! first asks `minecraft:gameplay/monsters_burn` at the body's position, and
//! the overworld's `minecraft:day` timeline (`data/minecraft/timeline/day.json`
//! in the pinned server jar) keys it `false` at tick 12542 and `true` at tick
//! 23460. So the burning hours are `[23460, 24000) ∪ [0, 12542)`, which holds
//! `day` (1000), `noon` (6000) **and `dusk` (12000)**, and not `night`
//! (13000), `midnight` (18000) or `dawn` (23000). [`hour_burns`] reads the
//! window, never a keyword list, and the constants carry the jar they were read
//! from. Measured on the pinned server in a built delve's own world: a
//! bare-headed zombie at `dusk` under open sky lost 12 of 20 health in 20 s;
//! the same body at `night` lost none.
//!
//! **Rain protects only where it falls.** The burn tick is skipped while
//! `isInWaterOrRain`, and "in rain" is `Level.isRainingAt`: raining, sky
//! visible, **and the biome at the cell precipitates rain** ([`precipitates_at`]).
//! Which biome a cell stands in, and whether it rains, is
//! [`crate::compiler::horizon`]'s one answer — the same one emission lays in
//! `generator-settings` — so the weather this proof reasons about is the weather
//! the build ships. Vanilla's `minecraft:the_void` never rains: measured on the
//! pinned server in a built delve's own world, `dusk` + `rain` over `the_void`
//! burned (20 → 7 and 20 → 15 health in 20 s) while the same cell painted
//! `minecraft:plains` did not burn at all. That is why a void delve lays its own
//! void biome, which rains; an ocean delve stands in `minecraft:ocean`, which
//! rains; a surround paints biomes that rain. A painted biome cold enough to
//! snow would not protect, and is taken to rain, which can only under-fire.
//!
//! **When the fight happens.** The daylight cycle is frozen (spec-0010), so the
//! declared state holds until a `set-time` / `set-weather` cuts it. A body can
//! burn in any state it stands in from the beat that puts it in the world —
//! the `spawn-wave` that seats a wave, the `unleash-actor` that wakes an actor —
//! until it dies, so the proof asks whether **some** state in that span burns
//! ([`Clock`]). The span is read off the quest DAG, the only order the
//! campaign declares:
//!
//! * the state **at** the beat: the last cut before it in its own bundle
//!   (fire order is `(at_ticks, declaration)`, a `sequence` step at its offset);
//!   else the final cut of every *latest* bundle that `depends_on` / `after`
//!   put strictly before it; else the declared state;
//! * every cut in a bundle **not** strictly before the beat — a concurrent
//!   quest, a later one — except, for a wave a `kill` objective adjudicates and
//!   no rest re-seats, the bundles at or after that objective, when the wave is
//!   already dead;
//! * every cut with no place in the DAG — a trigger, a trap, a dialogue option,
//!   a reaction bundle, a walk's `on_arrive` — because it can fire at any time.
//!
//! A beat with no place in the DAG itself (an approach trigger, an ambush, a
//! body standing from world init) can meet every reachable state. Time and
//! weather are cut independently and are paired as two sets, as
//! [`crate::compiler::light::reachable_time_weather`] pairs them. What is not
//! modelled: ordering a `requires_flags` gate imposes beyond `depends_on` and
//! `after` (a cut the flags put strictly before a beat is taken as concurrent),
//! and the state of the first ticks after a bundle whose `sequence` has not yet
//! reached its later steps.
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

use crate::compiler::failure::Failure;
use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{Campaign, Objective, QuestEffect, Verb, WorldTime, WorldWeather};

use crate::compiler::light::LightModel;
use crate::compiler::nav::{DEFAULT_FOLLOW_RANGE, World};
use crate::compiler::plan::Plan;
use delvewright_dsl::{DwCode, ExitTier};

/// `DW0496`: a body vanilla burns in daylight is staged for a fight whose ground
/// reaches open sky, in an hour and weather the fight can stand in that burn it,
/// with nothing on its head.
pub const DW_DAYLIGHT_BURNS_STAGING: DwCode = DwCode::new("DW0496", ExitTier::Build);

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
/// The vendored tag table now lives in [`crate::compiler::registry`] — it is vanilla
/// registry data, and `DW0452`/`DW0453` read it too.
pub fn burns_in_daylight(entity: &str) -> bool {
    !FIRE_IMMUNE.contains(&crate::compiler::registry::namespaced_entity(entity).as_str())
        && crate::compiler::registry::entity_in_tag(entity, BURN_IN_DAYLIGHT_TAG)
}

/// Whether a helmet stops this entity type's burn: everything except a phantom.
fn helmet_helps(entity: &str) -> bool {
    !HELMET_PROOF.contains(&crate::compiler::registry::namespaced_entity(entity).as_str())
}

/// Whether `equipment.head` is a remedy this rule may prescribe for `entity`:
/// a helmet stops its burn **and** its body draws a head slot. A gate that names
/// a remedy owes a check that the remedy is reachable, and `DW0898` refuses a
/// head piece on a body that shows none (a zombie horse, a zombie nautilus), so
/// the prescription is read from the same body table that refusal reads.
fn head_piece_is_a_remedy(entity: &str) -> bool {
    helmet_helps(entity)
        && delvewright_dsl::equipment::shows_slot(entity, delvewright_dsl::EquipSlot::Head)
}

/// Where the pinned `minecraft:day` timeline turns `minecraft:gameplay/monsters_burn`
/// off: tick 12542 of the day. Read from `data/minecraft/timeline/day.json` in
/// the pinned 1.21.11 server jar (`versions.toml` `[minecraft]`
/// `server_jar_sha256` `f83b8e09…dd1726`, the bundled
/// `META-INF/versions/1.21.11/server-1.21.11.jar`), keyframe
/// `{"ticks": 12542, "value": false}`.
const MONSTERS_BURN_OFF_AT: i64 = 12542;

/// Where it turns back on: tick 23460, keyframe `{"ticks": 23460, "value": true}`
/// of the same file.
const MONSTERS_BURN_ON_AT: i64 = 23460;

/// Whether the pinned game runs the sun-burn tick at this hour: the
/// `monsters_burn` window of the `minecraft:day` timeline, read by tick.
pub fn hour_burns(time: WorldTime) -> bool {
    let tick = time.daytime_ticks().rem_euclid(24_000);
    !(MONSTERS_BURN_OFF_AT..MONSTERS_BURN_ON_AT).contains(&tick)
}

/// The biome `cell` stands in: the surround rectangle painting it, else the
/// ground biome the generator lays ([`crate::compiler::horizon::ground_biome`]).
fn biome_at(plan: &Plan, cell: [i32; 3]) -> (String, bool) {
    if let Some(surround) = &plan.surround
        && let Some(rect) = surround
            .biome
            .iter()
            .find(|r| (0..3).all(|i| r.min[i] <= cell[i] && cell[i] <= r.max[i]))
    {
        let rains =
            crate::compiler::horizon::vanilla_precipitates(rect.biome).unwrap_or_else(|| {
                panic!(
                    "the surround paints `{}`, whose precipitation is unrecorded",
                    rect.biome
                )
            });
        return (rect.biome.to_string(), rains);
    }
    let ground = crate::compiler::horizon::ground_biome(plan.campaign, &plan.namespace);
    (ground.id, ground.precipitates)
}

/// Whether declared rain falls on `cell`: the biome there precipitates.
pub fn precipitates_at(plan: &Plan, cell: [i32; 3]) -> bool {
    biome_at(plan, cell).1
}

/// A place in the quest DAG an effect root has: an objective's completion
/// bundle or a quest's.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Pos {
    Objective { quest: String, objective: String },
    QuestComplete { quest: String },
}

impl Pos {
    fn quest(&self) -> &str {
        match self {
            Pos::Objective { quest, .. } | Pos::QuestComplete { quest } => quest,
        }
    }
}

/// When an effect fires inside its bundle: `(tick offset, pre-order index)`.
type FireKey = (u64, usize);

/// One cut of the delve's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut {
    Time(WorldTime),
    Weather(WorldWeather),
}

/// One DAG-placed bundle: every cut in it that fires at a known offset, and
/// every beat in it, each with its fire key.
#[derive(Default)]
struct Bundle {
    cuts: Vec<(FireKey, Cut)>,
    /// `(fire key, effect address)` for every effect in the bundle's timeline.
    effects: Vec<(FireKey, usize)>,
}

/// Where a body enters the fight.
#[derive(Clone, Debug)]
pub(crate) enum Beat {
    /// An effect in a DAG-placed bundle, at its fire key.
    Placed(Pos, FireKey),
    /// Anything with no place in the DAG: a trigger, an ambush, a trap, a
    /// reaction bundle, world init.
    Anywhere,
}

/// The states a delve's clock can be in, as two independent sets.
#[derive(Default, Debug)]
pub(crate) struct Sky {
    pub(crate) times: Vec<WorldTime>,
    pub(crate) weathers: Vec<WorldWeather>,
}

impl Sky {
    fn add(&mut self, cut: Cut) {
        match cut {
            Cut::Time(t) if !self.times.contains(&t) => self.times.push(t),
            Cut::Weather(w) if !self.weathers.contains(&w) => self.weathers.push(w),
            _ => {}
        }
    }
}

/// The delve's clock over the quest DAG (module docs, §3). Shared with
/// [`crate::compiler::engage`] (`DW0920`): one reading of which states a body
/// can stand in between entering a fight and leaving it.
pub(crate) struct Clock {
    declared: (WorldTime, WorldWeather),
    bundles: BTreeMap<Pos, Bundle>,
    /// Cuts with no place in the DAG, or no known offset in their bundle.
    anywhere: Vec<Cut>,
    /// Every quest's transitive `depends_on`.
    quest_ancestors: BTreeMap<String, BTreeSet<String>>,
    /// Every objective's transitive intra-quest `after`, keyed `(quest, objective)`.
    objective_ancestors: BTreeMap<(String, String), BTreeSet<String>>,
    /// Every effect the DAG-placed timelines reach, by address, with its place.
    placed: BTreeMap<usize, (Pos, FireKey)>,
}

fn cut_of(e: &QuestEffect) -> Option<Cut> {
    e.set_time()
        .map(Cut::Time)
        .or_else(|| e.set_weather().map(Cut::Weather))
}

fn addr(e: &QuestEffect) -> usize {
    e as *const QuestEffect as usize
}

/// Walk one bundle's timeline: direct effects at the bundle's own offset in
/// order, a `sequence` step at its `at_ticks`. Every other nested list (a
/// walk's `on_arrive`, a rest's `on_rest`, a checkpoint's `on_respawn`, a
/// stealth beat's `on_caught`) fires at no offset the bundle knows, so it is
/// left for the caller to classify as `anywhere`.
fn walk_timeline<'a>(
    list: &'a [QuestEffect],
    tick: u64,
    next: &mut usize,
    out: &mut Vec<(FireKey, &'a QuestEffect)>,
) {
    for e in list {
        out.push(((tick, *next), e));
        *next += 1;
        if let Verb::Sequence { steps } = &e.verb {
            for step in steps {
                walk_timeline(&step.effects, tick + u64::from(step.at_ticks), next, out);
            }
        }
    }
}

impl Clock {
    pub(crate) fn new(c: &Campaign) -> Self {
        let mut bundles: BTreeMap<Pos, Bundle> = BTreeMap::new();
        let mut anywhere: Vec<Cut> = Vec::new();
        let mut placed: BTreeMap<usize, (Pos, FireKey)> = BTreeMap::new();
        crate::compiler::plan::for_each_effect_root(c, &mut |site, effs| {
            let pos = match &site.root {
                crate::compiler::plan::EffectRoot::ObjectiveComplete { quest, objective } => {
                    Some(Pos::Objective {
                        quest: (*quest).to_string(),
                        objective: (*objective).to_string(),
                    })
                }
                crate::compiler::plan::EffectRoot::QuestComplete(quest) => {
                    Some(Pos::QuestComplete {
                        quest: quest.id.as_str().to_string(),
                    })
                }
                _ => None,
            };
            let mut timed: Vec<(FireKey, &QuestEffect)> = Vec::new();
            if pos.is_some() {
                walk_timeline(effs, 0, &mut 0, &mut timed);
            }
            let timed_at: BTreeMap<usize, FireKey> =
                timed.iter().map(|(k, e)| (addr(e), *k)).collect();
            for root in effs {
                root.visit_deep(&mut |e| {
                    let Some(key) = timed_at.get(&addr(e)).copied() else {
                        if let Some(cut) = cut_of(e) {
                            anywhere.push(cut);
                        }
                        return;
                    };
                    let pos = pos.clone().expect("only a placed bundle is timed");
                    let bundle = bundles.entry(pos.clone()).or_default();
                    if let Some(cut) = cut_of(e) {
                        bundle.cuts.push((key, cut));
                    }
                    bundle.effects.push((key, addr(e)));
                    placed.insert(addr(e), (pos, key));
                });
            }
        });
        // Dialogue options cut the clock as flat outcomes of a conversation,
        // never inside a quest bundle: they can fire whenever it is held.
        for tree in &c.dialogue.content.dialogues {
            for node in &tree.nodes {
                for opt in &node.options {
                    for e in &opt.effects {
                        if let Some(t) = e.set_time() {
                            anywhere.push(Cut::Time(t));
                        }
                        if let Some(w) = e.set_weather() {
                            anywhere.push(Cut::Weather(w));
                        }
                    }
                }
            }
        }
        let depends: BTreeMap<&str, Vec<&str>> = c
            .quest_plan
            .content
            .quests
            .iter()
            .map(|q| {
                (
                    q.id.as_str(),
                    q.depends_on.iter().map(|d| d.as_str()).collect(),
                )
            })
            .collect();
        let quest_ancestors = depends
            .keys()
            .map(|q| (q.to_string(), closure(q, &depends)))
            .collect();
        let mut objective_ancestors = BTreeMap::new();
        for q in &c.quests.content.quests {
            let after: BTreeMap<&str, Vec<&str>> = q
                .objectives
                .iter()
                .map(|o| {
                    (
                        o.id().as_str(),
                        o.after().iter().map(|a| a.as_str()).collect(),
                    )
                })
                .collect();
            for o in after.keys() {
                objective_ancestors.insert(
                    (q.id.as_str().to_string(), o.to_string()),
                    closure(o, &after),
                );
            }
        }
        Clock {
            declared: (c.world.content.time, c.world.content.weather),
            bundles,
            anywhere,
            quest_ancestors,
            objective_ancestors,
            placed,
        }
    }

    /// Where an effect sits: its DAG place and fire key, or [`Beat::Anywhere`].
    fn beat_of(&self, e: &QuestEffect) -> Beat {
        match self.placed.get(&addr(e)) {
            Some((pos, key)) => Beat::Placed(pos.clone(), *key),
            None => Beat::Anywhere,
        }
    }

    /// Whether bundle `x` has fired before bundle `p` can, by `depends_on`
    /// and `after` alone.
    fn strictly_before(&self, x: &Pos, p: &Pos) -> bool {
        if self
            .quest_ancestors
            .get(p.quest())
            .is_some_and(|a| a.contains(x.quest()))
        {
            return true;
        }
        if x.quest() != p.quest() {
            return false;
        }
        match (x, p) {
            (Pos::Objective { .. }, Pos::QuestComplete { .. }) => true,
            (
                Pos::Objective { objective: ox, .. },
                Pos::Objective {
                    quest,
                    objective: op,
                },
            ) => self
                .objective_ancestors
                .get(&(quest.clone(), op.clone()))
                .is_some_and(|a| a.contains(ox)),
            _ => false,
        }
    }

    /// Every state the delve can be in, whenever.
    fn everything(&self, c: &Campaign) -> Sky {
        let (times, weathers) = crate::compiler::light::reachable_time_weather(c);
        Sky { times, weathers }
    }

    /// Every state a body entering at one of `beats` can stand in before it
    /// dies. `dead_after`: the `kill` objectives that end a wave no rest
    /// re-seats.
    pub(crate) fn sky_for(&self, c: &Campaign, beats: &[Beat], dead_after: &[Pos]) -> Sky {
        let mut sky = Sky::default();
        for beat in beats {
            let Beat::Placed(p, key) = beat else {
                return self.everything(c);
            };
            for cut in &self.anywhere {
                sky.add(*cut);
            }
            // At the beat, per dimension.
            let own = self.bundles.get(p);
            for time in [true, false] {
                let same = |cut: &Cut| matches!(cut, Cut::Time(_)) == time;
                let local = own.and_then(|b| {
                    b.cuts
                        .iter()
                        .filter(|(k, cut)| k < key && same(cut))
                        .max_by_key(|(k, _)| *k)
                        .map(|(_, cut)| *cut)
                });
                if let Some(cut) = local {
                    sky.add(cut);
                    continue;
                }
                let before: Vec<&Pos> = self
                    .bundles
                    .iter()
                    .filter(|(x, b)| {
                        self.strictly_before(x, p) && b.cuts.iter().any(|(_, c)| same(c))
                    })
                    .map(|(x, _)| x)
                    .collect();
                let latest = before
                    .iter()
                    .filter(|x| !before.iter().any(|y| self.strictly_before(x, y)));
                let mut any = false;
                for x in latest {
                    if let Some((_, cut)) = self.bundles[*x]
                        .cuts
                        .iter()
                        .filter(|(_, c)| same(c))
                        .max_by_key(|(k, _)| *k)
                    {
                        sky.add(*cut);
                        any = true;
                    }
                }
                if !any {
                    sky.add(if time {
                        Cut::Time(self.declared.0)
                    } else {
                        Cut::Weather(self.declared.1)
                    });
                }
            }
            // After the beat, until the body dies.
            for (x, b) in &self.bundles {
                if dead_after
                    .iter()
                    .any(|k| k == x || self.strictly_before(k, x))
                {
                    continue;
                }
                if x == p {
                    for (k, cut) in &b.cuts {
                        if k > key {
                            sky.add(*cut);
                        }
                    }
                } else if !self.strictly_before(x, p) {
                    for (_, cut) in &b.cuts {
                        sky.add(*cut);
                    }
                }
            }
        }
        sky
    }
}

/// The transitive closure of `start` over `edges` (not including `start`).
fn closure(start: &str, edges: &BTreeMap<&str, Vec<&str>>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack: Vec<&str> = edges.get(start).cloned().unwrap_or_default();
    while let Some(n) = stack.pop() {
        if out.insert(n.to_string()) {
            stack.extend(edges.get(n).cloned().unwrap_or_default());
        }
    }
    out
}

/// The nearest sky-open cell within `radius` blocks of any cell in `from`, on
/// ground walk-reachable from `from`, where `burns_here` says the weather does
/// not protect the body — the shortest lure, the one worth naming in the
/// diagnostic.
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
    burns_here: &dyn Fn([i32; 3]) -> bool,
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
        .filter(|&cell| d2(cell) <= r2 && light.sky_open(cell) && burns_here(cell))
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
        if let delvewright_dsl::Verb::UnleashActor { actor: id, .. } = &eff.verb
            && id.as_str() == actor.id.as_str()
        {
            unleashed = true;
        }
    });
    unleashed
}

/// One staged body the proof looks at. Shared with
/// [`crate::compiler::engage`] (`DW0920`), which asks a different question of
/// the same population.
pub(crate) struct Staged {
    /// `wave/…` or `actor/…`, for the message.
    pub(crate) owner: String,
    /// What the campaign calls the encounter (`wave` / `actor`).
    pub(crate) kind: &'static str,
    /// The vanilla entity id as authored.
    pub(crate) entity: String,
    /// Cells the body is staged on.
    pub(crate) cells: Vec<[i32; 3]>,
    /// Its aggro radius in blocks.
    pub(crate) radius: u32,
    /// Does it already wear something on its head?
    helmeted: bool,
    /// Where it enters the fight.
    pub(crate) beats: Vec<Beat>,
    /// The `kill` objectives after which it is dead for good.
    pub(crate) dead_after: Vec<Pos>,
}

/// The state the sun reaches the body in, named in the message.
struct Exposure {
    cell: [i32; 3],
    time: WorldTime,
    weather: WorldWeather,
    /// The biome the cell stands in, when no rain falls in it.
    dry: Option<String>,
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
    blocks: &std::sync::Arc<BTreeMap<[i32; 3], String>>,
    spawns: &BTreeMap<String, Vec<[i32; 3]>>,
) -> Result<(), Failure> {
    let c = plan.campaign;
    let clock = Clock::new(c);
    let staged = collect_staged(plan, spawns, &clock);
    if staged.is_empty() {
        return Ok(());
    }
    let light = LightModel::from_shared(std::sync::Arc::clone(blocks));
    for body in &staged {
        if !burns_in_daylight(&body.entity) {
            continue;
        }
        if body.helmeted && helmet_helps(&body.entity) {
            continue;
        }
        let sky = clock.sky_for(c, &body.beats, &body.dead_after);
        // The witness names the declared state when it is one the fight can
        // burn in: that is the state the author wrote and the party plays in.
        let declared = (c.world.content.time, c.world.content.weather);
        let Some(&time) = (sky.times.contains(&declared.0) && hour_burns(declared.0))
            .then_some(&declared.0)
            .or_else(|| sky.times.iter().find(|&&t| hour_burns(t)))
        else {
            continue;
        };
        let clear = sky.weathers.contains(&WorldWeather::Clear);
        let Some(cell) = sky_within_reach(world, &light, &body.cells, body.radius, &|cell| {
            clear || !precipitates_at(plan, cell)
        }) else {
            continue;
        };
        let (biome, rains) = biome_at(plan, cell);
        let dry = !rains;
        let weather = if dry && sky.weathers.contains(&declared.1) {
            declared.1
        } else if clear {
            WorldWeather::Clear
        } else {
            sky.weathers[0]
        };
        return Err(Failure {
            code: DW_DAYLIGHT_BURNS_STAGING,
            message: burn_message(
                body,
                &Exposure {
                    cell,
                    time,
                    weather,
                    dry: (dry && weather != WorldWeather::Clear).then_some(biome),
                },
            ),
        });
    }
    Ok(())
}

/// Every staged body worth proving: wave stacks a `kill` objective adjudicates,
/// and actors the party can damage, each with the beats it enters the fight at.
/// Deterministic order (declaration order, waves then actors).
pub(crate) fn collect_staged(
    plan: &Plan,
    spawns: &BTreeMap<String, Vec<[i32; 3]>>,
    clock: &Clock,
) -> Vec<Staged> {
    let c = plan.campaign;
    let fought = killed_waves(c);
    // Every beat that seats a wave, spawns an actor or wakes one.
    let mut wave_beats: BTreeMap<&str, Vec<Beat>> = BTreeMap::new();
    let mut spawn_beats: BTreeMap<&str, Vec<Beat>> = BTreeMap::new();
    let mut unleash_beats: BTreeMap<&str, Vec<Beat>> = BTreeMap::new();
    delvewright_dsl::for_each_campaign_effect(c, &mut |_, _, e| {
        if let Some(w) = e.spawn_wave() {
            wave_beats
                .entry(w.as_str())
                .or_default()
                .push(clock.beat_of(e));
        }
        match &e.verb {
            Verb::SpawnActor { actor } => spawn_beats
                .entry(actor.as_str())
                .or_default()
                .push(clock.beat_of(e)),
            Verb::UnleashActor { actor } => unleash_beats
                .entry(actor.as_str())
                .or_default()
                .push(clock.beat_of(e)),
            _ => {}
        }
    });
    let mut out: Vec<Staged> = Vec::new();
    for w in &c.quests.content.waves {
        if !fought.contains(w.id.as_str()) {
            continue;
        }
        let Some(cells) = spawns.get(w.id.as_str()) else {
            continue;
        };
        let beats = wave_beats
            .get(w.id.as_str())
            .cloned()
            .unwrap_or_else(|| vec![Beat::Anywhere]);
        let dead_after = if w.respawns_on_rest {
            Vec::new()
        } else {
            kill_objectives(c, w.id.as_str())
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
                beats: beats.clone(),
                dead_after: dead_after.clone(),
            });
        }
    }
    for a in &c.quests.content.actors {
        if !fightable_actor(c, a) {
            continue;
        }
        let Some(pos) = plan.body_point(delvewright_dsl::BodyRef::Actor(a)) else {
            continue;
        };
        let mut beats = unleash_beats
            .get(a.id.as_str())
            .cloned()
            .unwrap_or_default();
        if a.vulnerable {
            // A damageable puppet is a fight from the moment it stands: from
            // its `spawn-actor`, or from world init when nothing spawns it.
            beats.extend(
                spawn_beats
                    .get(a.id.as_str())
                    .cloned()
                    .unwrap_or_else(|| vec![Beat::Anywhere]),
            );
        }
        if beats.is_empty() {
            beats.push(Beat::Anywhere);
        }
        out.push(Staged {
            owner: a.id.as_str().to_string(),
            kind: "actor",
            entity: a.entity.clone(),
            cells: vec![pos],
            radius: stack_radius(a.attributes),
            helmeted: a.equipment.as_ref().is_some_and(|e| e.head.is_some()),
            beats,
            dead_after: Vec::new(),
        });
    }
    out
}

/// The `kill` objectives that adjudicate `wave`, as DAG places.
pub(crate) fn kill_objectives(c: &Campaign, wave: &str) -> Vec<Pos> {
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            if let Objective::Kill { wave: w, .. } = o
                && w.as_str() == wave
            {
                out.push(Pos::Objective {
                    quest: q.id.as_str().to_string(),
                    objective: o.id().as_str().to_string(),
                });
            }
        }
    }
    out
}

/// The diagnostic text: what is staged, the state and cell the sun gets in at,
/// and the two fixes — plus the one that is forbidden.
fn burn_message(body: &Staged, exposure: &Exposure) -> String {
    let Staged {
        owner,
        kind,
        entity,
        cells,
        radius,
        ..
    } = body;
    let at = cells[0];
    let Exposure {
        cell,
        time,
        weather,
        dry,
    } = exposure;
    let remedy = if head_piece_is_a_remedy(entity) {
        "Give this stack `equipment.head` (any head item — vanilla damages the helmet instead \
         of igniting the mob, and the compiler emits drop chance 0 so it can never be farmed), \
         or roof the ground the fight happens on."
    } else if helmet_helps(entity) {
        "Roof the ground the fight happens on, or stage this encounter somewhere the sky does \
         not reach. `equipment.head` is NOT a fix for this species: its body draws no head \
         slot, so a head piece is refused (`DW0898`)."
    } else {
        "Roof the ground the fight happens on, or stage this encounter somewhere the sky does \
         not reach. `equipment.head` is NOT a fix for this species."
    };
    let sanctioned = if head_piece_is_a_remedy(entity) {
        "; the sanctioned fix is recorded on the `equipment.head` DSL field itself"
    } else {
        ""
    };
    let rain = match dry {
        Some(biome) => format!(
            " The `{}` the delve declares does not protect it: no rain falls in `{biome}`, \
             the biome this cell stands in, so the pinned game never counts the body as wet.",
            weather.keyword()
        ),
        None => String::new(),
    };
    format!(
        "{kind} `{owner}` stages `{entity}` at [{}, {}, {}], and vanilla burns that species in \
         daylight (`#minecraft:burn_in_daylight`). The fight can stand in `{}` with `{}`, an \
         hour the pinned game burns undead in (its `minecraft:day` timeline keeps \
         `monsters_burn` on from tick {MONSTERS_BURN_ON_AT} to tick {MONSTERS_BURN_OFF_AT}), \
         and open sky stands at [{}, {}, {}] — walkable ground inside this stack's own \
         {radius}-block aggro radius.{rain} A player retreating there is still its target, so \
         the fight the party is meant to have is decided by the sun instead: this is the \
         Barrowmere gate yard, where two of three footmen died to sunlight in under twenty \
         seconds with every proof green. Fix the content: {remedy} Do NOT use `set-time` — the \
         delve's hour is a pacing decision the author made, and moving it to save a mob spends \
         a beat{sanctioned}.",
        at[0],
        at[1],
        at[2],
        time.keyword(),
        weather.keyword(),
        cell[0],
        cell[1],
        cell[2],
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

    /// The burning hours are the pinned timeline's `monsters_burn` window, and
    /// `dusk` is inside it.
    #[test]
    fn the_burning_hours_are_the_pinned_timeline_window() {
        assert!(hour_burns(WorldTime::Day));
        assert!(hour_burns(WorldTime::Noon));
        assert!(hour_burns(WorldTime::Dusk));
        assert!(!hour_burns(WorldTime::Night));
        assert!(!hour_burns(WorldTime::Midnight));
        assert!(!hour_burns(WorldTime::Dawn));
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
