//! **The muster** — the wave a delve declares, read off the bodies it actually
//! seated.
//!
//! ## What this exists to catch
//!
//! A wave mob's `max_health`, `attack_damage`, `equipment` and `name` are
//! written into one `summon` line and never looked at again. If an attribute
//! component is dropped by the pinned codec, if a slot key is spelled the way an
//! older version spelled it, if a name renders as a literal `'{"text":…}'` — the
//! delve boots, the fight happens, and nothing anywhere says the declaration did
//! not reach the body. That failure has happened in this repository twice
//! already, in exactly this shape: 1.21.11 silently ignores `HandItems` on a
//! `summon`, and it silently drops the legacy `PatrolTarget` compound. Both were
//! found by hand, on a spike, long after they shipped.
//!
//! The muster closes that. It is a probe the compiler emits beside the census
//! (`wave_census_*`), on the same anchored chat channel, that walks the wave's
//! own tag and states — per live body — what the **server** says its attributes,
//! equipment and name are. The harness compares that against what this module
//! derived from the declaration. A declared number that never reached the body
//! is a finding with the body's own reading beside it.
//!
//! It is a reading, never a fight: nothing here asks whether the wave is hard,
//! or whether anything can beat it. The ladder verifies mechanism.
//!
//! ## Why the identity half is a bitmask
//!
//! Attributes are numbers and cross the chat channel as numbers. A name and an
//! item id are not, and inventing a string channel for them would mean parsing
//! player-visible text out of chat — the thing the marker grammar exists to stop.
//!
//! So the compiler bakes the question into the probe instead of carrying the
//! answer out of it. Every *identity fact* a wave declares (this stack is named
//! `X`; this stack's head slot holds `Y`) becomes one `execute if data entity @s
//! {…}` in the probe, and one bit in a single integer. The harness reads the
//! integer and decodes it against the fact list this module also wrote into
//! `combat-plan.json`, so both sides name the same facts in the same order,
//! derived once, here.
//!
//! ## Matching bodies to stacks
//!
//! A wave may seat two stacks of ONE entity kind that differ only in name —
//! `wave/drowned-choir` seats three Choristers and one Precentor. Nothing on a
//! live body says which stack it came from, so the harness does not ask: it
//! compares the **multiset** of `(type, mask, attributes)` it read against the
//! multiset this module declares. A missing helmet moves one body out of its
//! profile and the comparison names it, whichever stack it belonged to.

use std::collections::BTreeMap;

use delvewright_dsl::{EquipSlot, Wave, WaveMob};
use serde_json::{Value, json};

use crate::compiler::plan;
use crate::compiler::registry::ItemCombatRegistry;

/// Fixed-point scale every muster number crosses the chat channel at.
///
/// Three decimal places, not the census's two: `movement_speed` is declared in
/// blocks per tick (a zombie's is `0.23`) and two places cannot tell `0.230` from
/// `0.234`. The widest reading a delve can produce — a 280-health boss — is
/// `280000`, four orders of magnitude inside a scoreboard's `i32`.
pub const MUSTER_SCALE: i64 = 1000;

/// What a holder reads when the probe deliberately did not ask.
///
/// Every attribute the probe reads is non-negative, so a negative value cannot be
/// a reading. `attack_damage` and `follow_range` exist on a Mob and not on every
/// LivingEntity, and Mojang publishes no per-entity attribute table
/// (`DW0475`'s rule) — so the probe asks for them only where the wave DECLARES
/// them, and says so here rather than reporting a zero nobody measured.
pub const MUSTER_UNREAD: i64 = -1;

/// The widest fact list one entity kind may carry, in bits.
///
/// A scoreboard holder is an `i32` and the mask is built by addition, so bit 31
/// is the sign. Thirty facts is four stacks of one kind each wearing a full set
/// of armour, both hands and a name; nothing in any campaign is near it. A wave
/// that exceeds it has its extra facts dropped from the probe and says so in the
/// plan, because a silently short mask would read as a passing check.
pub const MUSTER_FACT_LIMIT: usize = 30;

/// One thing the declaration says is true of a body, phrased so the server can be
/// asked it directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusterFact {
    /// `name`, or `equipment.<nbt slot>` — what the fact is about.
    pub about: String,
    /// The declared value, in the form the plan states it (an item id, or the
    /// name's authored text).
    pub value: String,
    /// The `execute if data entity @s {…}` predicate that tests it.
    pub predicate: String,
}

impl MusterFact {
    /// How the plan (and therefore every finding) names this fact.
    pub fn label(&self) -> String {
        format!("{}={}", self.about, self.value)
    }
}

/// How the plan names a declared NAME, and why it is not the name.
///
/// `combat-plan.json` is language-neutral: a `--lang zh-cn` bake swaps every
/// authored string before emission, so a plan carrying the text would differ
/// between two builds of one campaign that must be identical outside the
/// string-bearing files (`cli::lang_build_localizes_only_strings_and_is_deterministic`).
/// The ordinal is the index of this distinct name among the ones this entity kind
/// declares in this wave, in declaration order, and a finding names the stack
/// beside it — enough to resolve against the campaign, in any language.
fn name_token(ordinal: usize) -> String {
    format!("#{ordinal}")
}

/// One entity kind standing in this wave, and what the probe asks of it.
#[derive(Debug, Clone)]
pub struct MusterType {
    /// The vanilla entity id, as `summon` writes it.
    pub entity: String,
    /// Every identity fact any stack of this kind declares, in a fixed order —
    /// bit `i` of a body's mask is `facts[i]`.
    pub facts: Vec<MusterFact>,
    /// Facts this wave declares that did not fit in the mask.
    pub dropped_facts: Vec<String>,
    /// Does EVERY stack of this kind declare `attack_damage`? Only then is it
    /// read: an undeclared one may not carry the attribute at all.
    pub reads_attack_damage: bool,
    /// The same question for `follow_range`.
    pub reads_follow_range: bool,
}

/// One declared stack, as the multiset comparison expects to find it.
#[derive(Debug, Clone)]
pub struct MusterProfile {
    /// Index into [`Muster::types`].
    pub type_index: usize,
    /// How many bodies of this profile the wave seats.
    pub count: u32,
    /// The mask a body of this stack must carry: every one of its own identity
    /// facts set, and no other.
    pub mask: i64,
    /// How a finding names this stack.
    pub label: String,
    /// Declared `max_health`, or `None` when the stack leaves it vanilla (read
    /// as telemetry, compared against nothing).
    pub max_health: Option<f64>,
    /// Declared `attack_damage`.
    pub attack_damage: Option<f64>,
    /// Declared `movement_speed`.
    pub movement_speed: Option<f64>,
    /// Declared `follow_range`.
    pub follow_range: Option<f64>,
    /// Armour points the declared equipment contributes, from the vendored
    /// 1.21.11 item table. The body's own `armor` attribute must be at least
    /// this: a mob's BASE armour is not in any published data, so the sum is a
    /// floor and never an equality — and a floor is enough, because the defect
    /// this catches is gear that never arrived.
    pub armor_at_least: f64,
    /// The same floor for `armor_toughness`.
    pub armor_toughness_at_least: f64,
}

/// Everything the muster knows about one wave.
#[derive(Debug, Clone)]
pub struct Muster {
    /// The wave's id.
    pub wave: String,
    /// Entity kinds, in declaration order.
    pub types: Vec<MusterType>,
    /// Declared stacks, in declaration order.
    pub profiles: Vec<MusterProfile>,
    /// How many bodies the wave seats in total.
    pub bodies: u32,
}

impl Muster {
    /// How many declared facts this probe puts a question to — the binding count
    /// a run artifact states its findings against.
    ///
    /// Counted, never hand-written: the seating itself (one per stack), each
    /// identity fact, and each declared attribute the probe reads back.
    pub fn checked(&self) -> usize {
        let mut n = self.profiles.len(); // one seated-count question per stack
        for p in &self.profiles {
            let t = &self.types[p.type_index];
            n += p.mask.count_ones() as usize;
            if p.max_health.is_some() {
                n += 1;
            }
            if p.movement_speed.is_some() {
                n += 1;
            }
            if p.attack_damage.is_some() && t.reads_attack_damage {
                n += 1;
            }
            if p.follow_range.is_some() && t.reads_follow_range {
                n += 1;
            }
            if p.armor_at_least > 0.0 {
                n += 1;
            }
            if p.armor_toughness_at_least > 0.0 {
                n += 1;
            }
        }
        n
    }
}

/// Build the muster model for one wave.
///
/// `equipment_of` resolves a stack's EFFECTIVE equipment — the slots the `summon`
/// line actually writes, which includes the compiler's armed-mob main-hand
/// default. The check is against what the emitter claims to have put on the body,
/// so the two must come from one function; `emit` owns that resolution and hands
/// it in here.
pub fn muster(
    wave: &Wave,
    equipment_of: &dyn Fn(&WaveMob) -> Vec<(EquipSlot, String)>,
    items: &ItemCombatRegistry,
    name_predicate: &dyn Fn(&str) -> String,
) -> Muster {
    // Entity kind -> index, in declaration order (never a map's own order: ADR-0006).
    let mut order: Vec<String> = Vec::new();
    for mob in &wave.mobs {
        if !order.iter().any(|e| e == &mob.entity) {
            order.push(mob.entity.clone());
        }
    }
    let mut types: Vec<MusterType> = order
        .iter()
        .map(|entity| {
            let stacks: Vec<&WaveMob> = wave.mobs.iter().filter(|m| &m.entity == entity).collect();
            MusterType {
                entity: entity.clone(),
                facts: Vec::new(),
                dropped_facts: Vec::new(),
                reads_attack_damage: stacks
                    .iter()
                    .all(|m| m.attributes.and_then(|a| a.attack_damage).is_some()),
                reads_follow_range: stacks
                    .iter()
                    .all(|m| m.attributes.and_then(|a| a.follow_range).is_some()),
            }
        })
        .collect();
    let index_of: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, e)| (e.as_str(), i))
        .collect();

    // Pass one: collect every identity fact, in declaration order, deduplicated.
    for mob in &wave.mobs {
        let t = &mut types[index_of[mob.entity.as_str()]];
        if let Some(name) = &mob.name {
            // The fact is named by ORDINAL, never by the text (see `name_token`).
            // What the probe ASKS is the component itself, built by
            // `name_predicate` from the same string the summon writes.
            let predicate = format!("{{CustomName:{}}}", name_predicate(name));
            let ordinal = t
                .facts
                .iter()
                .filter(|f| f.about == "name")
                .count();
            if !t.facts.iter().any(|f| f.predicate == predicate) {
                push_fact(
                    t,
                    MusterFact {
                        about: "name".to_string(),
                        value: name_token(ordinal),
                        predicate,
                    },
                );
            }
        }
        for (slot, item) in equipment_of(mob) {
            push_fact(
                t,
                MusterFact {
                    about: format!("equipment.{}", slot.nbt()),
                    value: item.clone(),
                    predicate: format!(
                        "{{equipment:{{{}:{{id:\"{}\"}}}}}}",
                        slot.nbt(),
                        item
                    ),
                },
            );
        }
    }

    // Pass two: the per-stack expectation, now that every bit position is fixed.
    let mut profiles: Vec<MusterProfile> = Vec::new();
    for (stack, mob) in wave.mobs.iter().enumerate() {
        let ti = index_of[mob.entity.as_str()];
        let equipment = equipment_of(mob);
        let mut mask: i64 = 0;
        if let Some(name) = &mob.name {
            let predicate = format!("{{CustomName:{}}}", name_predicate(name));
            if let Some(i) = types[ti].facts.iter().position(|f| f.predicate == predicate) {
                mask |= 1i64 << i;
            }
        }
        let mut armor = 0.0;
        let mut toughness = 0.0;
        for (slot, item) in &equipment {
            if let Some(b) = bit_of(&types[ti], &format!("equipment.{}", slot.nbt()), item) {
                mask |= b;
            }
            // Only worn armour contributes; a held sword's `armor` is zero in the
            // table anyway, and naming the slots keeps that a statement rather
            // than a coincidence.
            if matches!(
                slot,
                EquipSlot::Head | EquipSlot::Chest | EquipSlot::Legs | EquipSlot::Feet
            ) {
                if let Some(stats) = items.get(item) {
                    armor += stats.armor;
                    toughness += stats.armor_toughness;
                }
            }
        }
        let attrs = mob.attributes;
        profiles.push(MusterProfile {
            type_index: ti,
            count: mob.count,
            mask,
            // Language-neutral, like every other field here: the stack index is
            // what a reader resolves against the campaign document.
            label: format!("{} × {} (stack {stack})", mob.count, mob.entity),
            max_health: attrs.and_then(|a| a.max_health),
            attack_damage: attrs.and_then(|a| a.attack_damage),
            movement_speed: attrs.and_then(|a| a.movement_speed),
            follow_range: attrs.and_then(|a| a.follow_range),
            armor_at_least: armor,
            armor_toughness_at_least: toughness,
        });
    }

    Muster {
        wave: wave.id.as_str().to_string(),
        types,
        profiles,
        bodies: wave.mobs.iter().map(|m| m.count).sum(),
    }
}

/// Add a fact to a kind's list unless it is already there, or the mask is full.
fn push_fact(t: &mut MusterType, fact: MusterFact) {
    if t.facts.iter().any(|f| f.about == fact.about && f.value == fact.value) {
        return;
    }
    if t.facts.len() >= MUSTER_FACT_LIMIT {
        let label = fact.label();
        if !t.dropped_facts.contains(&label) {
            t.dropped_facts.push(label);
        }
        return;
    }
    t.facts.push(fact);
}

/// The mask bit a `(about, value)` fact occupies for this kind, if the probe asks
/// it at all.
fn bit_of(t: &MusterType, about: &str, value: &str) -> Option<i64> {
    t.facts
        .iter()
        .position(|f| f.about == about && f.value == value)
        .map(|i| 1i64 << i)
}

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

/// Scratch holders. Shared across waves, which is safe for the same reason the
/// census's are: one muster is one atomic function call.
const H_SEQ: &str = "#wmus_seq";
const H_COUNTED: &str = "#wmus_n";
const H_ALL: &str = "#wmus_all";
const H_TYPE: &str = "#wmus_t";
const H_MASK: &str = "#wmus_k";
const H_MAX_HEALTH: &str = "#wmus_mh";
const H_ARMOR: &str = "#wmus_ar";
const H_TOUGHNESS: &str = "#wmus_at";
const H_SPEED: &str = "#wmus_ms";
const H_ATTACK: &str = "#wmus_ad";
const H_FOLLOW: &str = "#wmus_fr";
const H_ATTACK_EFFECTIVE: &str = "#wmus_ade";

/// The holders the summary line carries, in wire order.
pub const MUSTER_SUMMARY_HOLDERS: [&str; 3] = [H_SEQ, H_COUNTED, H_ALL];

/// The holders one body's line carries, in wire order.
pub const MUSTER_BODY_HOLDERS: [&str; 10] = [
    H_SEQ,
    H_TYPE,
    H_MASK,
    H_MAX_HEALTH,
    H_ARMOR,
    H_TOUGHNESS,
    H_SPEED,
    H_ATTACK,
    H_FOLLOW,
    H_ATTACK_EFFECTIVE,
];

/// The function name the harness calls to take one muster of `wave_id`.
pub fn muster_fn(wave_id: &str) -> String {
    format!("wave_muster_{}", plan::safe_local(wave_id))
}

/// The per-kind accumulation function.
fn muster_one_fn(wave_id: &str, type_index: usize) -> String {
    format!("wave_muster_one_{}_{type_index}", plan::safe_local(wave_id))
}

/// The function that fells one body of `wave_id`, credited to the party.
///
/// **Staging, not a fight.** The ladder does not swing at a wave: it reads the
/// bodies, then removes them and goes on to what the kill drives. The removal is
/// a `player_attack` `by @p` rather than a `kill` because the wiring under test —
/// `on_kill`, the countdown, a declared drop — pays on a PLAYER's kill, and a
/// removal that credits nobody would skip all of it and leave the step reading
/// green over machinery that never ran.
pub fn strike_fn(wave_id: &str) -> String {
    format!("wave_strike_{}", plan::safe_local(wave_id))
}

/// The function that wounds every body of `wave_id` without felling one.
///
/// The die-retry stage (spec-0023 §1) proves that a death mid-fight is safe, and
/// "mid-fight" is a state of the WAVE: bodies below their own `max_health`, which
/// a faithful re-seat must replace. One attributed point of damage puts the wave
/// in that state exactly, with no fencing anywhere in it.
pub fn chip_fn(wave_id: &str) -> String {
    format!("wave_chip_{}", plan::safe_local(wave_id))
}

/// How hard the staged blow hits.
///
/// Larger than any health a delve can declare after armour and resistance take
/// their cut, so one blow is one body. Not `kill`: see [`strike_fn`].
const STRIKE_DAMAGE: &str = "100000";

/// Every function the muster and the staged removal need, as
/// `(name, lines)` — the caller joins them.
pub fn functions(ns: &str, m: &Muster) -> Vec<(String, Vec<String>)> {
    let wave_id = m.wave.as_str();
    let tag = plan::wave_tag(wave_id);
    let mut out: Vec<(String, Vec<String>)> = Vec::new();

    for (ti, t) in m.types.iter().enumerate() {
        let mut body = vec![
            format!("scoreboard players add {H_COUNTED} dw.sys 1"),
            format!("scoreboard players set {H_TYPE} dw.sys {ti}"),
            format!("scoreboard players set {H_MASK} dw.sys 0"),
        ];
        for (i, fact) in t.facts.iter().enumerate() {
            body.push(format!(
                "execute if data entity @s {} run scoreboard players add {H_MASK} dw.sys {}",
                fact.predicate,
                1i64 << i
            ));
        }
        // A declared attribute is a BASE value, and `attribute … get` returns the
        // TOTAL after every modifier: a summoned zombie carries vanilla's own
        // random spawn bonus on `movement_speed` (0.23 base reads 0.276 total) and
        // its main-hand weapon's `attack_damage` modifier (2.0 base reads 5.0 with
        // a wooden sword). Comparing a declaration against a total is a check that
        // fails on every correct body, so what the declaration set is read with
        // `base get` and what the player actually meets is read separately.
        for (holder, attr) in [
            (H_MAX_HEALTH, "max_health"),
            (H_SPEED, "movement_speed"),
        ] {
            body.push(format!(
                "execute store result score {holder} dw.sys run attribute @s minecraft:{attr} \
                 base get {MUSTER_SCALE}"
            ));
        }
        // Armour is the EFFECTIVE total on purpose: it is not declared anywhere, and
        // the question it answers is whether the declared gear reached the body —
        // a mob's base armour is not in any published data, so the worn pieces'
        // contribution is a floor under the total and never an equality.
        for (holder, attr) in [(H_ARMOR, "armor"), (H_TOUGHNESS, "armor_toughness")] {
            body.push(format!(
                "execute store result score {holder} dw.sys run attribute @s minecraft:{attr} \
                 get {MUSTER_SCALE}"
            ));
        }
        for (holder, attr, read) in [
            (H_ATTACK, "attack_damage", t.reads_attack_damage),
            (H_FOLLOW, "follow_range", t.reads_follow_range),
        ] {
            body.push(format!(
                "scoreboard players set {holder} dw.sys {MUSTER_UNREAD}"
            ));
            if read {
                body.push(format!(
                    "execute store result score {holder} dw.sys run attribute @s \
                     minecraft:{attr} base get {MUSTER_SCALE}"
                ));
            }
        }
        // …and what the body actually swings with, weapon included. Telemetry, not
        // a check: `docs/notes/shield-and-guard-fight.md` measured a Guard declared
        // `attack_damage: 6.0` hitting for 11.0 with its iron sword, and nothing in
        // any artifact said so.
        body.push(format!(
            "scoreboard players set {H_ATTACK_EFFECTIVE} dw.sys {MUSTER_UNREAD}"
        ));
        if t.reads_attack_damage {
            body.push(format!(
                "execute store result score {H_ATTACK_EFFECTIVE} dw.sys run attribute @s \
                 minecraft:attack_damage get {MUSTER_SCALE}"
            ));
        }
        body.push(format!("tellraw @a {}", body_component(ns, wave_id)));
        out.push((muster_one_fn(wave_id, ti), body));
    }

    let mut top = vec![
        format!("scoreboard players add {H_SEQ} dw.sys 1"),
        format!("scoreboard players set {H_COUNTED} dw.sys 0"),
        format!("scoreboard players set {H_ALL} dw.sys 0"),
        format!("execute as @e[tag={tag}] run scoreboard players add {H_ALL} dw.sys 1"),
    ];
    for (ti, t) in m.types.iter().enumerate() {
        top.push(format!(
            "execute as @e[tag={tag},type={}] run function {ns}:{}",
            t.entity,
            muster_one_fn(wave_id, ti)
        ));
    }
    top.push(format!("tellraw @a {}", summary_component(ns, wave_id)));
    out.push((muster_fn(wave_id), top));

    out.push((
        strike_fn(wave_id),
        vec![format!(
            "execute as @e[tag={tag},limit=1,sort=nearest] run damage @s {STRIKE_DAMAGE} \
             minecraft:player_attack by @p"
        )],
    ));
    out.push((
        chip_fn(wave_id),
        vec![format!(
            "execute as @e[tag={tag}] run damage @s 1 minecraft:player_attack by @p"
        )],
    ));
    out
}

/// The summary line: the sequence, how many bodies of a DECLARED kind were read,
/// and how many carry the wave's tag at all. The two disagreeing is a body of a
/// kind the wave never declared.
fn summary_component(ns: &str, wave_id: &str) -> Value {
    component(ns, plan::MARKER_TOKEN_MUSTER, wave_id, &MUSTER_SUMMARY_HOLDERS)
}

/// One body's line.
fn body_component(ns: &str, wave_id: &str) -> Value {
    component(
        ns,
        plan::MARKER_TOKEN_MUSTER_BODY,
        wave_id,
        &MUSTER_BODY_HOLDERS,
    )
}

/// The census's own component shape, one token further on. Kept here rather than
/// reached for across `emit` because the holder lists above are this module's.
fn component(ns: &str, token: &str, wave_id: &str, holders: &[&str]) -> Value {
    let mut extra: Vec<Value> = Vec::new();
    for h in holders {
        extra.push(json!({ "score": { "name": *h, "objective": "dw.sys" } }));
        extra.push(json!({ "text": " " }));
    }
    extra.pop();
    extra.push(json!({ "text": "]" }));
    json!({
        "text": format!("[dw:{token} {ns} {wave_id} "),
        "color": "dark_gray",
        "extra": extra
    })
}

// ---------------------------------------------------------------------------
// The plan the harness reads
// ---------------------------------------------------------------------------

/// The `muster` block of one encounter in `validation/combat-plan.json`.
pub fn to_json(ns: &str, m: &Muster) -> Value {
    json!({
        // Function ids in the census's own convention (`<ns>:<fn>`): the harness
        // calls what the plan NAMES and never re-derives `safe_local`.
        "probe": format!("{ns}:{}", muster_fn(&m.wave)),
        "strike": format!("{ns}:{}", strike_fn(&m.wave)),
        "chip": format!("{ns}:{}", chip_fn(&m.wave)),
        "scale": MUSTER_SCALE,
        "unread": MUSTER_UNREAD,
        "bodies": m.bodies,
        "checked": m.checked(),
        "types": m
            .types
            .iter()
            .map(|t| json!({
                "entity": t.entity,
                "facts": t.facts.iter().map(MusterFact::label).collect::<Vec<_>>(),
                "dropped_facts": t.dropped_facts,
                "reads_attack_damage": t.reads_attack_damage,
                "reads_follow_range": t.reads_follow_range,
            }))
            .collect::<Vec<_>>(),
        "profiles": m
            .profiles
            .iter()
            .map(|p| json!({
                "type": p.type_index,
                "count": p.count,
                "mask": p.mask,
                "label": p.label,
                "max_health": p.max_health,
                "attack_damage": p.attack_damage,
                "movement_speed": p.movement_speed,
                "follow_range": p.follow_range,
                "armor_at_least": p.armor_at_least,
                "armor_toughness_at_least": p.armor_toughness_at_least,
            }))
            .collect::<Vec<_>>(),
    })
}
