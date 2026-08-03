//! Compile-time combat winnability (spec-0023 §2) — the arithmetic half of the
//! three combat proofs.
//!
//! The founding invariant is that a delve is machine-verified before the owner
//! sees it; spec-0023 changes **what** the machine asserts about a fight. It no
//! longer pretends "the average player can win" was ever provable. What it does
//! prove, per mandatory encounter, is that the fight is *structurally* winnable:
//!
//! - every hostile is **damageable** ([`DW_UNDAMAGEABLE`]);
//! - every hostile's body is **reachable** — a player has somewhere to stand and
//!   swing from ([`DW_UNREACHABLE`]);
//! - **time-to-kill is bounded** by the best kit the party can field
//!   ([`DW_TTK_OVER_BUDGET`]), or the compiler says out loud that it could not
//!   compute the bound ([`DW_TTK_UNPROVEN`]);
//! - no **unavoidable single hit** on the critical path exceeds player max HP
//!   ([`DW_UNAVOIDABLE_LETHAL`]);
//! - the party carries **some** sustain ([`DW_NO_SUSTAIN`]).
//!
//! # Every number comes from Mojang, or is refused
//!
//! Weapon damage, armour points and food nutrition are read from the vendored
//! `minecraft:attribute_modifiers` / `minecraft:food` default components
//! (`registry::ItemCombatRegistry`); whether a damage type is reduced by armour
//! or scaled by difficulty comes from the vendored damage-type registry
//! (`registry::DamageTypeRegistry`). Mojang publishes **no** per-entity default
//! attributes, so a mob's base health simply is not knowable at build time — and
//! rather than invent a health table (the "invented precision" this codebase
//! refuses for `nav::DEFAULT_FOLLOW_RANGE` and `clearance::MODEL_MARGIN`), the
//! numeric TTK bound runs only where the campaign declares
//! `attributes.max_health`, and [`DW_TTK_UNPROVEN`] names every stack that opted
//! out. That is the whole reason "the bound failed" and "the bound could not be
//! computed" are two different diagnostics.
//!
//! # The Easy-halving trap
//!
//! `WorldDifficulty`'s doc comment states the Easy formula `min(dmg/2+1, dmg)`,
//! and reading only that would make this module wrong by 2× in the *lenient*
//! direction. Difficulty scaling is a property of the DAMAGE TYPE: a type scales
//! only when its `scaling` field says so for the attacker at hand, and
//! `damage-players` emits a bare `/damage <target> <amount> <type>` with **no
//! attacker at all**. Eight of the nine types the DSL exposes are
//! `when_caused_by_living_non_player` — i.e. unscaled here. Only
//! `minecraft:explosion` (`always`) is scaled. The arithmetic states which rule
//! it applied, every time.
//!
//! # Deliberately coarse
//!
//! spec-0023 puts human dodge/aim skill explicitly out of scope, and forbids the
//! compiler from scaling content. Every bound here is a sanity bound, sized so
//! that only a genuinely broken encounter trips it; taste stays with the author
//! and the owner's playtest. Where a fact is unknown the answer is "unproven",
//! never a guess in either direction — an encounter is never failed on an
//! assumption.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{
    Campaign, Diagnostic, EncounterTier, QuestEffect, Wave, WaveMob, WorldDifficulty,
};
use serde_json::{Value, json};

use crate::nav::{World, entity_dims};
use crate::plan::{self, Plan, Step};
use crate::registry::{DamageTypeRegistry, ItemCombatRegistry};

/// `DW0470`: a hostile the party is *required* to kill can never be damaged.
pub const DW_UNDAMAGEABLE: &str = "DW0470";

/// `DW0471`: a hostile the party is required to kill has no cell to be fought
/// from — its body is walled in.
pub const DW_UNREACHABLE: &str = "DW0471";

/// `DW0472`: a mandatory encounter's declared health outlasts the best kit the
/// party can field, by the [`TTK_BUDGET_HITS`] sanity bound.
pub const DW_TTK_OVER_BUDGET: &str = "DW0472";

/// `DW0473`: an unavoidable scripted hit on the critical path kills a
/// full-health player outright.
pub const DW_UNAVOIDABLE_LETHAL: &str = "DW0473";

/// `DW0474`: a campaign with mandatory combat hands the party no sustain at all.
pub const DW_NO_SUSTAIN: &str = "DW0474";

/// `DW0475`: (warning) the numeric time-to-kill bound could not be computed.
pub const DW_TTK_UNPROVEN: &str = "DW0475";

/// The vanilla player's `minecraft:max_health` base value. The DSL exposes no
/// player-attribute surface at all, so this is not a default — it is the only
/// value a delve can ship.
pub const PLAYER_MAX_HEALTH: f64 = 20.0;

/// The vanilla player's `minecraft:attack_speed` base value, which a held
/// weapon's (negative) `attack_speed` modifier subtracts from. Used only to put
/// an indicative *duration* next to the hit count in a diagnostic — never in the
/// gate itself, which counts swings and therefore needs no timing model.
pub const PLAYER_BASE_ATTACK_SPEED: f64 = 4.0;

/// How many full-damage swings one player may need to clear a single mandatory
/// encounter before the compiler calls the fight structurally unwinnable.
///
/// Deliberately enormous. An iron sword (5 damage) clearing eight 20-HP zombies
/// is 32 swings; this bound is reached only around 2000 effective HP against the
/// party's best weapon — i.e. a stack that is not a hard fight but an arithmetic
/// mistake. spec-0023 asks for "a sanity bound, not a balance opinion", and the
/// compiler is explicitly forbidden from having balance opinions.
pub const TTK_BUDGET_HITS: u32 = 400;

/// The `minecraft:resistance` amplifier at which incoming damage reduction
/// reaches 100%. Vanilla resistance reduces by 20% per level, so amplifier 4
/// (level V) is total immunity to everything outside
/// `#minecraft:bypasses_resistance` — a fact the emitter already relies on for
/// its PackTest scaffolding, and the only way a wave mob can be spelled
/// unkillable.
pub const RESISTANCE_IMMUNE_AMPLIFIER: u32 = 4;

/// A build failure raised by the winnability proof (exit 3, like the `nav` /
/// `clearance` build errors it sits beside).
#[derive(Debug)]
pub struct CombatError {
    /// The stable diagnostic code.
    pub code: &'static str,
    /// Human-readable explanation: the arithmetic, the formula it used, and how
    /// to retune without re-deriving any of it.
    pub message: String,
}

/// One mandatory encounter: a wave a `kill` objective on the compiled critical
/// path requires the party to clear.
#[derive(Debug, Clone)]
pub struct Encounter {
    /// The wave id (`wave/<kebab>`).
    pub wave_id: String,
    /// The `kill` objective the wave completes.
    pub objective_id: String,
    /// Index into `plan.critical_path` of the `kill` step.
    pub step: usize,
    /// What the content bills the fight as (`ordinary` unless declared).
    pub tier: EncounterTier,
    /// The wave anchor cell.
    pub pos: [i32; 3],
    /// Total mob count.
    pub count: i32,
    /// Does resting re-seat this wave (spec-0016 §1)?
    pub respawns_on_rest: bool,
    /// The checkpoint the party respawns at while this encounter is live, if the
    /// campaign has set one by then.
    pub checkpoint: Option<[i32; 3]>,
}

/// Cheap gate: does this campaign have any mandatory combat at all? A campaign
/// with no `kill` step never enters the winnability pass and is byte-identical
/// to before spec-0023.
pub fn has_encounters(plan: &Plan) -> bool {
    plan.critical_path
        .iter()
        .any(|s| matches!(s, Step::Kill { .. }))
}

/// The effective world difficulty: what the campaign declared, else the
/// compiler's historical derivation (`easy` once any wave exists, `peaceful`
/// otherwise) — the same rule the emitter ships to `server.properties`.
pub fn effective_difficulty(c: &Campaign) -> WorldDifficulty {
    c.world.content.difficulty.unwrap_or({
        if c.quests.content.waves.is_empty() {
            WorldDifficulty::Peaceful
        } else {
            WorldDifficulty::Easy
        }
    })
}

/// Apply the world difficulty's player-damage multiplier.
///
/// This is only ever correct **together with** the damage type's `scaling`
/// field; call it through [`incoming_damage`], never directly, or you reproduce
/// the Easy-halving trap this module's header describes.
fn difficulty_scaled(amount: f64, difficulty: WorldDifficulty) -> f64 {
    match difficulty {
        // `min(dmg / 2 + 1, dmg)`.
        WorldDifficulty::Easy => (amount / 2.0 + 1.0).min(amount),
        WorldDifficulty::Normal => amount,
        WorldDifficulty::Hard => amount * 3.0 / 2.0,
        // Rejected by `DW0468` long before any build reaches here; the arm
        // exists so this match needs no catch-all that could hide a new variant.
        WorldDifficulty::Peaceful => amount,
    }
}

/// What a declared `damage-players` amount actually lands as, and the one-line
/// statement of how that number was reached.
///
/// Returns `None` when the compiler declines to adjudicate: an armour-respecting
/// damage type's landed amount depends on what the player is wearing at that
/// beat, and a kit is a flat item list with no slots, so the compiler does not
/// know. Declining is the false-positive-free choice — the DSL's default type,
/// `generic`, bypasses armour, which is where scripted consequence lives.
pub fn incoming_damage(
    amount: u32,
    damage_type: &str,
    difficulty: WorldDifficulty,
    types: &DamageTypeRegistry,
) -> Option<(f64, String)> {
    let facts = types.get(damage_type)?;
    if !facts.bypasses_armor {
        return None;
    }
    let raw = f64::from(amount);
    if facts.scales_without_attacker() {
        let scaled = difficulty_scaled(raw, difficulty);
        Some((
            scaled,
            format!(
                "{raw} x difficulty({}) = {scaled} (damage type {damage_type} has \
                 scaling=\"always\", so the difficulty multiplier applies)",
                difficulty.token()
            ),
        ))
    } else {
        Some((
            raw,
            format!(
                "{raw}, UNSCALED: damage type {damage_type} has scaling=\"{}\", and \
                 `damage-players` emits `/damage` with no attacker — so difficulty \
                 ({}) does not touch it",
                facts.scaling,
                difficulty.token()
            ),
        ))
    }
}

/// The best single melee hit any one class can land, and the class/item it came
/// from. `None` when no kit carries an item with an `attack_damage` attribute —
/// which means *unknown*, not zero: a bow deals real damage and has no such
/// attribute (its damage is projectile code, in no vanilla data at all).
fn best_melee_hit(c: &Campaign, items: &ItemCombatRegistry) -> Option<(f64, String, String)> {
    let mut best: Option<(f64, String, String)> = None;
    for class in &c.classes.content.classes {
        for kit in &class.kit {
            let Some(stats) = items.get(&kit.item) else {
                continue;
            };
            if stats.attack_damage <= 0.0 {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|(d, _, _)| stats.attack_damage > *d)
            {
                best = Some((
                    stats.attack_damage,
                    class.id.as_str().to_string(),
                    kit.item.clone(),
                ));
            }
        }
    }
    best
}

/// Swings per second for a weapon: the player's base attack speed plus the
/// item's (negative) modifier. Context only — never part of a gate.
fn swings_per_second(item: &str, items: &ItemCombatRegistry) -> f64 {
    let modifier = items.get(item).map(|s| s.attack_speed).unwrap_or(0.0);
    (PLAYER_BASE_ATTACK_SPEED + modifier).max(0.1)
}

/// The incoming-damage multiplier a mob's declared `effects` give it, and the
/// effect that set it. 0.0 means total immunity.
fn mob_damage_multiplier(mob: &WaveMob) -> (f64, Option<(String, u32)>) {
    let mut worst: Option<(String, u32)> = None;
    let mut multiplier = 1.0_f64;
    for e in &mob.effects {
        if e.effect != "minecraft:resistance" && e.effect != "resistance" {
            continue;
        }
        let reduction = f64::from(e.amplifier + 1) * 0.2;
        let m = (1.0 - reduction).max(0.0);
        if m < multiplier {
            multiplier = m;
            worst = Some((e.effect.clone(), e.amplifier));
        }
    }
    (multiplier, worst)
}

/// Collect the mandatory encounters of a compiled plan, in critical-path order.
pub fn encounters(plan: &Plan) -> Vec<Encounter> {
    let mut out = Vec::new();
    for (i, step) in plan.critical_path.iter().enumerate() {
        let Step::Kill {
            objective_id,
            wave_id,
            pos,
            count,
            ..
        } = step
        else {
            continue;
        };
        let wave = plan::wave_of(plan.campaign, wave_id);
        // The checkpoint governing a death at this encounter is the last one the
        // campaign fires at or before the step (spec-0012 checkpoints are
        // party-wide and monotonic by quest order).
        let checkpoint = plan
            .checkpoints
            .iter()
            .rfind(|cp| cp.fire_step <= i)
            .map(|cp| cp.pos);
        out.push(Encounter {
            wave_id: wave_id.clone(),
            objective_id: objective_id.clone(),
            step: i,
            tier: wave.and_then(|w| w.tier).unwrap_or_default(),
            pos: *pos,
            count: *count,
            respawns_on_rest: wave.is_some_and(|w| w.respawns_on_rest),
            checkpoint,
        });
    }
    out
}

/// The mandatory-encounter wave ids, for the checks that walk waves directly.
fn mandatory_waves(plan: &Plan) -> BTreeSet<String> {
    encounters(plan).into_iter().map(|e| e.wave_id).collect()
}

/// Is there a cell a player could stand on and swing from, adjacent to this
/// body?
///
/// Deliberately **local**: a Chebyshev-1 ring around each column the body
/// occupies, over the elevations the body spans. It says nothing about global
/// connectivity, which is what makes it free of the false positives a
/// reachability flood would produce — a room legitimately opened later by a gate
/// or a shortcut is not disconnected, it is merely shut for now, and
/// `check_critical_path` already owns that question. What this catches is the
/// thing no other proof does: a body with no fighting cell beside it at all,
/// walled in on every side.
fn has_fighting_cell(world: &World, cell: [i32; 3], entity: &str) -> bool {
    let (width, height) = entity_dims(entity);
    // How many cells the body's footprint spans on each horizontal axis, and how
    // many its height spans vertically — both rounded up, so a wide body's
    // shoulders are included.
    let span = (width.ceil() as i32 - 1).max(0);
    let rise = (height.ceil() as i32 - 1).max(0);
    for dx in -1 - span..=1 + span {
        for dz in -1 - span..=1 + span {
            for dy in -1..=rise {
                let c = [cell[0] + dx, cell[1] + dy, cell[2] + dz];
                if c == cell {
                    continue;
                }
                if world.is_standable(c) {
                    return true;
                }
            }
        }
    }
    false
}

/// One seated hostile: the wave that owns it, the entity, and the exact cell the
/// datapack will summon it on.
type SeatedHostile = (String, String, [i32; 3]);

/// Pair each mandatory wave's mobs with the standable cells `plan_wave_spawns`
/// seated them on, in declaration order (each stack takes `count` seats).
fn seated_hostiles(
    waves: &[Wave],
    mandatory: &BTreeSet<String>,
    placements: &BTreeMap<String, Vec<[i32; 3]>>,
) -> Vec<SeatedHostile> {
    let mut out = Vec::new();
    for wave in waves {
        if !mandatory.contains(wave.id.as_str()) {
            continue;
        }
        let Some(cells) = placements.get(wave.id.as_str()) else {
            continue;
        };
        let mut seat = 0usize;
        for mob in &wave.mobs {
            for _ in 0..mob.count {
                if let Some(cell) = cells.get(seat) {
                    out.push((wave.id.as_str().to_string(), mob.entity.clone(), *cell));
                }
                seat += 1;
            }
        }
    }
    out
}

/// The `DW0470` finding list: mandatory hostiles nothing in a kit can hurt.
fn undamageable_hostiles(waves: &[Wave], mandatory: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for wave in waves {
        if !mandatory.contains(wave.id.as_str()) {
            continue;
        }
        for mob in &wave.mobs {
            let (multiplier, source) = mob_damage_multiplier(mob);
            if multiplier > 0.0 {
                continue;
            }
            let (effect, amplifier) = source.expect("a zero multiplier always names its effect");
            out.push(format!(
                "  {} x{} in {} carries `{effect}` amplifier {amplifier} (level {}): \
                 100% damage reduction, i.e. total immunity",
                mob.entity,
                mob.count,
                wave.id.as_str(),
                amplifier + 1,
            ));
        }
    }
    out
}

/// The `DW0471` finding list: seated hostiles with no cell to be fought from.
fn unreachable_hostiles(world: &World, seated: &[SeatedHostile]) -> Vec<String> {
    seated
        .iter()
        .filter(|(_, entity, cell)| !has_fighting_cell(world, *cell, entity))
        .map(|(wave, entity, cell)| {
            format!(
                "  {entity} of {wave} is seated at [{}, {}, {}] with no standable cell \
                 anywhere around it",
                cell[0], cell[1], cell[2]
            )
        })
        .collect()
}

/// The full winnability pass. Errors abort the build (exit 3); warnings ride
/// along on a successful one.
///
/// `placements` is the per-wave seated spawn cells `plan_wave_spawns` already
/// proved standable (`DW0312`) — the exact cells the datapack will summon on.
pub fn check_winnability(
    plan: &Plan,
    world: &World,
    placements: &BTreeMap<String, Vec<[i32; 3]>>,
) -> Result<Vec<Diagnostic>, CombatError> {
    let c = plan.campaign;
    let items = ItemCombatRegistry::v1_21_11();
    let types = DamageTypeRegistry::v1_21_11();
    let difficulty = effective_difficulty(c);
    let mandatory = mandatory_waves(plan);
    let mut warnings = Vec::new();

    // ---- DW0470: every required hostile can be hurt at all -----------------
    let undamageable = undamageable_hostiles(&c.quests.content.waves, &mandatory);
    if !undamageable.is_empty() {
        return Err(CombatError {
            code: DW_UNDAMAGEABLE,
            message: format!(
                "a hostile the party is REQUIRED to kill can never be damaged, so its `kill` \
                 objective can never complete and the delve soft-locks:\n{}\n\nResistance \
                 reduces incoming damage by 20% per level, so amplifier {RESISTANCE_IMMUNE_AMPLIFIER} \
                 (level {}) is total immunity to everything outside \
                 `#minecraft:bypasses_resistance` — nothing in a player's kit can reach it. Fix: \
                 lower the amplifier to at most {} (level {}), which is 80% reduction and still an \
                 extremely tanky elite, or drop the effect and put the durability in \
                 `attributes.max_health` instead, where it is a number the winnability arithmetic \
                 can bound. Do NOT delete the `kill` objective to silence this — an unkillable mob \
                 standing in the room is still an unkillable mob.",
                undamageable.join("\n"),
                RESISTANCE_IMMUNE_AMPLIFIER + 1,
                RESISTANCE_IMMUNE_AMPLIFIER - 1,
                RESISTANCE_IMMUNE_AMPLIFIER
            ),
        });
    }

    // ---- DW0471: every required hostile can be reached ---------------------
    let seated = seated_hostiles(&c.quests.content.waves, &mandatory, placements);
    let unreachable = unreachable_hostiles(world, &seated);
    if !unreachable.is_empty() {
        return Err(CombatError {
            code: DW_UNREACHABLE,
            message: format!(
                "a hostile the party is REQUIRED to kill has nowhere to be fought from — its \
                 body is walled in, so no player can stand within reach of it and the `kill` \
                 objective can never complete:\n{}\n\nThe check is deliberately local: it asks \
                 only whether ANY cell adjacent to the body's own footprint is standable, so a \
                 room merely shut behind a gate or a shortcut is untouched (that is \
                 `check_critical_path`'s question, not this one). Fix: move the wave `anchor` \
                 into open floor, or carve the pocket the mobs are seated in. Do NOT widen the \
                 wave's spawn search to hide it — the mobs would simply be seated somewhere the \
                 author never staged.",
                unreachable.join("\n")
            ),
        });
    }

    // ---- DW0472 / DW0475: time to kill -------------------------------------
    let best = best_melee_hit(c, &items);
    let mut unproven: Vec<String> = Vec::new();
    let encounters = encounters(plan);
    for enc in &encounters {
        let Some(wave) = plan::wave_of(c, &enc.wave_id) else {
            continue;
        };
        let mut declared_ehp = 0.0_f64;
        let mut undeclared: Vec<&WaveMob> = Vec::new();
        for mob in &wave.mobs {
            let Some(max_health) = mob.attributes.and_then(|a| a.max_health) else {
                undeclared.push(mob);
                continue;
            };
            let (multiplier, _) = mob_damage_multiplier(mob);
            // A resistance-bearing mob soaks proportionally more of the same
            // swings; multiplier 0 was already rejected as DW0470.
            declared_ehp += f64::from(mob.count) * max_health / multiplier;
        }
        match (&best, declared_ehp > 0.0) {
            (Some((hit, class, item)), true) => {
                let hits = (declared_ehp / hit).ceil() as u64;
                if hits > u64::from(TTK_BUDGET_HITS) {
                    let sps = swings_per_second(item, &items);
                    #[allow(clippy::cast_precision_loss)]
                    let seconds = hits as f64 / sps;
                    return Err(CombatError {
                        code: DW_TTK_OVER_BUDGET,
                        message: format!(
                            "encounter {} ({}) outlasts the best kit the party can field:\n\
                             \n  declared effective HP  {declared_ehp}\n  \
                             best single hit        {hit} ({item}, from {class})\n  \
                             swings needed          ceil({declared_ehp} / {hit}) = {hits}\n  \
                             budget                 {TTK_BUDGET_HITS} swings\n  \
                             indicative duration    {hits} / {sps:.2} swings-per-second \
                             = {seconds:.0}s of uninterrupted attacking, by ONE player\n\n\
                             The gate counts SWINGS, not seconds, because swing damage is \
                             Mojang's own item data while timing depends on charge discipline \
                             the compiler cannot model; the duration line is context only. Only \
                             the weapon's own `attack_damage` modifier is counted — the \
                             player's base fist damage is deliberately excluded, so the real \
                             fight is always at least this fast. Fix: lower \
                             `attributes.max_health`, cut the stack `count`, or put a stronger \
                             weapon in a kit. Do NOT raise the budget: {TTK_BUDGET_HITS} swings \
                             is already far past any fight a human would sit through, so \
                             crossing it means the numbers are wrong, not that the fight is \
                             hard.",
                            enc.wave_id, enc.objective_id
                        ),
                    });
                }
            }
            (None, _) => unproven.push(format!(
                "  {}: no class kit carries an item with an `attack_damage` attribute, so the \
                 party's damage output is unknown (a bow's damage is projectile code and \
                 appears in no vanilla data — absence is not zero)",
                enc.wave_id
            )),
            (Some(_), false) => {}
        }
        if !undeclared.is_empty() && best.is_some() {
            let names: Vec<String> = undeclared
                .iter()
                .map(|m| format!("{} x{}", m.entity, m.count))
                .collect();
            unproven.push(format!(
                "  {}: {} declare no `attributes.max_health`",
                enc.wave_id,
                names.join(", ")
            ));
        }
    }
    if !unproven.is_empty() {
        warnings.push(Diagnostic::warning(
            DW_TTK_UNPROVEN,
            "quests",
            "/content/waves",
            format!(
                "the time-to-kill bound could not be computed for every mandatory encounter, so \
                 those fights ship with the structural proofs only (damageable, reachable, \
                 wired) and no arithmetic:\n{}\n\nThis is a statement about vanilla data, not \
                 about the content: Mojang publishes no per-entity default attributes, so a \
                 mob's base health is genuinely unknown at build time and the compiler refuses \
                 to invent a health table. Declare `attributes.max_health` on the stack to opt \
                 the encounter into the numeric bound (`DW0472`). Warning tier on purpose — an \
                 encounter left on vanilla stats is legitimate, the author just has to see that \
                 nothing arithmetic was proven about it.",
                unproven.join("\n")
            ),
        ));
    }

    // ---- DW0473: no unavoidable one-shot on the critical path --------------
    let mut lethal: Vec<String> = Vec::new();
    for (quest_index, quest) in c.quests.content.quests.iter().enumerate() {
        let mut bundles: Vec<(String, &Vec<QuestEffect>)> = vec![(
            format!("/content/quests/{quest_index}/on_complete"),
            &quest.on_complete,
        )];
        for (objective, effects) in &quest.on_objective_complete {
            bundles.push((
                format!(
                    "/content/quests/{quest_index}/on_objective_complete/{}",
                    objective.as_str()
                ),
                effects,
            ));
        }
        for (path, effects) in bundles {
            let mut found = Vec::new();
            collect_unconditional_damage(effects, &path, &mut found);
            for (where_, amount, damage_type) in found {
                let Some((landed, arithmetic)) =
                    incoming_damage(amount, &damage_type, difficulty, &types)
                else {
                    continue;
                };
                if landed < PLAYER_MAX_HEALTH {
                    continue;
                }
                lethal.push(format!(
                    "  {where_}: {arithmetic} >= {PLAYER_MAX_HEALTH} max HP"
                ));
            }
        }
    }
    if !lethal.is_empty() {
        return Err(CombatError {
            code: DW_UNAVOIDABLE_LETHAL,
            message: format!(
                "an UNAVOIDABLE scripted hit on the critical path kills a full-health player \
                 outright:\n{}\n\nThese fire from a quest's own effect bundle — the party \
                 completes an objective and the damage lands, with nothing to dodge and no \
                 decision to make — so a one-shot here is not difficulty, it is a scripted \
                 death. spec-0023 allows one-shots that a player can AVOID; the telegraph and \
                 saturation rules of spec-0016/0022 govern those, and trap payloads, stealth \
                 `on_caught` bundles and dialogue-option effects are all deliberately outside \
                 this check for exactly that reason. Fix: lower the `amount` below \
                 {PLAYER_MAX_HEALTH}, or move the consequence onto a beat the party can play \
                 around (a trap with a telegraph, a caught-in-stealth reaction). Note the \
                 arithmetic above states the damage type's own scaling rule — do NOT assume the \
                 Easy halving applies: `damage-players` emits `/damage` with no attacker, so \
                 only a `scaling=\"always\"` type is ever scaled.",
                lethal.join("\n")
            ),
        });
    }

    // ---- DW0474: the party carries some sustain ----------------------------
    if !encounters.is_empty() && !has_any_sustain(c, &items) {
        warnings.push(Diagnostic::warning(
            DW_NO_SUSTAIN,
            "classes",
            "/content/classes",
            format!(
                "this campaign has {} mandatory combat encounter(s) and hands the party no \
                 sustain at all: no class kit, `give-item` effect or `loot` container anywhere \
                 carries an item with a `minecraft:food` component. Natural regeneration stops \
                 the moment the hunger bar drops below 18, so after the first fight the party's \
                 health only ever goes down. Fix: put food in the kits, or stock a container on \
                 the route. Warning tier because the fight budget a party actually needs \
                 depends on play the compiler is forbidden to model (spec-0023 \"Out of \
                 scope\") — the finding here is the literal zero, which is a design fact rather \
                 than a balance opinion.",
                encounters.len()
            ),
        ));
    }

    Ok(warnings)
}

/// Walk an effect bundle for `damage-players` hits the party cannot dodge.
///
/// Descends `sequence` steps (a timeline is still the same unconditional
/// bundle), and deliberately stops at every reaction list — `on_respawn`,
/// `on_caught`, `on_arrive` — because those fire in response to something a
/// player did or failed to do, which is precisely the avoidable case spec-0023
/// leaves to the telegraph rules. A `within` zone likewise makes the hit
/// positional, so it is skipped: standing elsewhere is the counterplay.
fn collect_unconditional_damage(
    effects: &[QuestEffect],
    path: &str,
    out: &mut Vec<(String, u32, String)>,
) {
    for (i, e) in effects.iter().enumerate() {
        match e {
            QuestEffect::DamagePlayers {
                amount,
                within,
                damage_type,
                ..
            } => {
                if within.is_some() {
                    continue;
                }
                let id = damage_type
                    .map(|k| k.id().to_string())
                    .unwrap_or_else(|| "minecraft:generic".to_string());
                out.push((format!("{path}/{i}"), *amount, id));
            }
            QuestEffect::Sequence { steps } => {
                for (j, step) in steps.iter().enumerate() {
                    collect_unconditional_damage(
                        &step.effects,
                        &format!("{path}/{i}/steps/{j}/effects"),
                        out,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Does anything the party can get its hands on carry a `minecraft:food`
/// component? Kits, `give-item` effects (at any nesting depth) and `loot`
/// containers all count.
fn has_any_sustain(c: &Campaign, items: &ItemCombatRegistry) -> bool {
    let is_food = |id: &str| items.get(id).is_some_and(|s| s.nutrition > 0.0);
    if c.classes
        .content
        .classes
        .iter()
        .any(|class| class.kit.iter().any(|k| is_food(&k.item)))
    {
        return true;
    }
    if c.quests
        .content
        .loot
        .iter()
        .any(|l| l.items.iter().any(|i| is_food(&i.item)))
    {
        return true;
    }
    let mut given = false;
    let mut walk = |effects: &[QuestEffect]| {
        let mut stack: Vec<&QuestEffect> = effects.iter().collect();
        while let Some(e) = stack.pop() {
            match e {
                QuestEffect::GiveItem { item, .. } if is_food(item) => given = true,
                QuestEffect::Sequence { steps } => {
                    stack.extend(steps.iter().flat_map(|s| s.effects.iter()));
                }
                _ => {}
            }
        }
    };
    for q in &c.quests.content.quests {
        walk(&q.on_complete);
        for effects in q.on_objective_complete.values() {
            walk(effects);
        }
    }
    for t in c.quests.content.all_triggers() {
        walk(&t.effects);
    }
    given
}

/// The validation-only combat plan the bot ladder reads (spec-0023 §1/§3/§4).
///
/// Lives under `validation/` like the waypoint export — excluded from the
/// shipped delve image, so declaring an encounter tier or running the die-retry
/// stage can never change a shipped byte. Emitted only when the campaign has a
/// mandatory encounter, so a combat-free delve's output is unchanged entirely.
pub fn combat_plan_json(plan: &Plan, encounters: &[Encounter]) -> Value {
    let difficulty = effective_difficulty(plan.campaign);
    let entries: Vec<Value> = encounters
        .iter()
        .map(|e| {
            let mut o = json!({
                "wave": e.wave_id,
                "objective": e.objective_id,
                "step": e.step,
                "tier": e.tier.token(),
                "pos": [e.pos[0], e.pos[1], e.pos[2]],
                "count": e.count,
                "respawns_on_rest": e.respawns_on_rest,
            });
            if let Some(cp) = e.checkpoint {
                o["checkpoint"] = json!([cp[0], cp[1], cp[2]]);
            }
            o
        })
        .collect();
    json!({
        "version": plan.campaign.world.dsl_version,
        "campaign_id": plan.namespace,
        "difficulty": difficulty.token(),
        "encounters": entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easy_does_not_halve_a_scripted_generic_hit() {
        // The trap this module exists to avoid: `minecraft:generic` is
        // `when_caused_by_living_non_player`, and `/damage` has no attacker.
        let types = DamageTypeRegistry::v1_21_11();
        let (landed, why) =
            incoming_damage(40, "minecraft:generic", WorldDifficulty::Easy, &types).unwrap();
        assert_eq!(landed, 40.0);
        assert!(why.contains("UNSCALED"), "{why}");
    }

    #[test]
    fn the_difficulty_formula_is_the_documented_one() {
        // min(20/2 + 1, 20) = 11 on easy; 20 * 3/2 = 30 on hard.
        assert_eq!(difficulty_scaled(20.0, WorldDifficulty::Easy), 11.0);
        assert_eq!(difficulty_scaled(20.0, WorldDifficulty::Normal), 20.0);
        assert_eq!(difficulty_scaled(20.0, WorldDifficulty::Hard), 30.0);
    }

    #[test]
    fn no_damage_players_type_is_both_scaled_and_adjudicated() {
        // A finding worth pinning: of the nine types the DSL exposes, the ONE
        // whose `scaling` is "always" (`explosion`) is also the one armour
        // reduces — so difficulty never moves a number this check adjudicates.
        // Whichever way that pair changes at a future MC pin, this test says so.
        let types = DamageTypeRegistry::v1_21_11();
        for kind in [
            "minecraft:generic",
            "minecraft:magic",
            "minecraft:wither",
            "minecraft:on_fire",
            "minecraft:drown",
            "minecraft:freeze",
            "minecraft:fall",
            "minecraft:lightning_bolt",
            "minecraft:explosion",
        ] {
            let facts = types
                .get(kind)
                .expect("the DSL's curated types are vanilla");
            assert!(
                !(facts.bypasses_armor && facts.scales_without_attacker()),
                "{kind} is now both adjudicated and difficulty-scaled — DW0473's \
                 arithmetic must start applying the multiplier"
            );
        }
        assert!(
            incoming_damage(20, "minecraft:explosion", WorldDifficulty::Easy, &types).is_none()
        );
    }

    #[test]
    fn an_armour_respecting_type_is_not_adjudicated() {
        let types = DamageTypeRegistry::v1_21_11();
        // `lightning_bolt` respects armour; what lands depends on the worn kit,
        // which a slotless kit list does not state.
        assert!(
            incoming_damage(
                100,
                "minecraft:lightning_bolt",
                WorldDifficulty::Normal,
                &types
            )
            .is_none()
        );
    }

    #[test]
    fn resistance_five_is_total_immunity() {
        let mob = WaveMob {
            entity: "minecraft:zombie".to_string(),
            count: 1,
            name: None,
            attributes: None,
            effects: vec![delvewright_dsl::MobEffect {
                effect: "minecraft:resistance".to_string(),
                amplifier: RESISTANCE_IMMUNE_AMPLIFIER,
            }],
            equipment: None,
        };
        let (multiplier, source) = mob_damage_multiplier(&mob);
        assert_eq!(multiplier, 0.0);
        assert_eq!(source.unwrap().1, RESISTANCE_IMMUNE_AMPLIFIER);
    }

    #[test]
    fn resistance_four_still_lets_damage_through() {
        let mob = WaveMob {
            entity: "minecraft:zombie".to_string(),
            count: 1,
            name: None,
            attributes: None,
            effects: vec![delvewright_dsl::MobEffect {
                effect: "minecraft:resistance".to_string(),
                amplifier: RESISTANCE_IMMUNE_AMPLIFIER - 1,
            }],
            equipment: None,
        };
        let (multiplier, _) = mob_damage_multiplier(&mob);
        assert!((multiplier - 0.2).abs() < 1e-9, "{multiplier}");
    }

    /// An occupancy view holding nothing but the given solid cells.
    fn occ(solid: BTreeSet<[i32; 3]>) -> crate::assembled::Occupancy {
        crate::assembled::Occupancy {
            solid,
            tall: BTreeSet::new(),
            use_gates: BTreeSet::new(),
            flooded: BTreeSet::new(),
            partial: BTreeMap::new(),
        }
    }

    /// A wave with one mob of `entity`, for the pure-rule tests.
    fn wave(
        id: &str,
        entity: &str,
        effects: Vec<delvewright_dsl::MobEffect>,
    ) -> delvewright_dsl::Wave {
        delvewright_dsl::Wave {
            id: delvewright_dsl::WaveId(id.to_string()),
            anchor: delvewright_dsl::AnchorId("anchor/pit".to_string()),
            mobs: vec![WaveMob {
                entity: entity.to_string(),
                count: 1,
                name: None,
                attributes: None,
                effects,
                equipment: None,
            }],
            respawns_on_rest: false,
            lane: None,
            summon: None,
            tier: None,
        }
    }

    #[test]
    fn a_walled_in_hostile_is_dw0471() {
        // A 1x1 pocket: the mob's own cell has a floor, so `plan_wave_spawns`
        // (DW0312) is happy — and there is nowhere at all for a player to stand
        // and swing from, which is the gap this code closes.
        let mut solid = BTreeSet::new();
        for dx in -1..=1 {
            for dz in -1..=1 {
                for dy in -1..=3 {
                    solid.insert([dx, dy, dz]);
                }
            }
        }
        // Carve the pocket itself (the mob stands on [0,-1,0]).
        solid.remove(&[0, 0, 0]);
        solid.remove(&[0, 1, 0]);
        let world = World::from_occupancy(occ(solid));
        let seated = vec![(
            "wave/pit".to_string(),
            "minecraft:zombie".to_string(),
            [0, 0, 0],
        )];
        let found = unreachable_hostiles(&world, &seated);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(DW_UNREACHABLE, "DW0471");

        // …and the same mob with one open cell beside it is fine.
        let mut solid2 = BTreeSet::new();
        for dx in -2..=2 {
            for dz in -2..=2 {
                solid2.insert([dx, -1, dz]);
            }
        }
        let open = World::from_occupancy(occ(solid2));
        assert!(unreachable_hostiles(&open, &seated).is_empty());
    }

    #[test]
    fn only_a_mandatory_wave_is_held_to_dw0470() {
        let waves = vec![
            wave(
                "wave/required",
                "minecraft:zombie",
                vec![delvewright_dsl::MobEffect {
                    effect: "minecraft:resistance".to_string(),
                    amplifier: RESISTANCE_IMMUNE_AMPLIFIER,
                }],
            ),
            wave(
                "wave/optional",
                "minecraft:zombie",
                vec![delvewright_dsl::MobEffect {
                    effect: "minecraft:resistance".to_string(),
                    amplifier: RESISTANCE_IMMUNE_AMPLIFIER,
                }],
            ),
        ];
        let mandatory: BTreeSet<String> = ["wave/required".to_string()].into_iter().collect();
        let found = undamageable_hostiles(&waves, &mandatory);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("wave/required"), "{found:?}");
        assert_eq!(DW_UNDAMAGEABLE, "DW0470");
    }

    #[test]
    fn a_bow_has_no_attack_damage_attribute_in_vanilla_data() {
        // Pins the reason `best_melee_hit` returning None means "unknown", not
        // "the party deals no damage" — the distinction DW0472 vs DW0475 rests on.
        let items = ItemCombatRegistry::v1_21_11();
        assert!(items.get("minecraft:bow").is_none());
        assert_eq!(
            items.get("minecraft:iron_sword").unwrap().attack_damage,
            5.0
        );
        assert_eq!(items.get("minecraft:cooked_beef").unwrap().nutrition, 8.0);
    }
}
