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

use crate::failure::Failure;
use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{
    Actor, Campaign, Diagnostic, EffectSite, EncounterTier, QuestEffect, Wave, WaveMob,
    WorldDifficulty, for_each_campaign_effect,
};
use serde_json::{Value, json};

use crate::nav::{World, entity_dims};
use crate::plan::{self, Plan, Step, safe_local};
use crate::registry::{DamageTypeRegistry, ItemCombatRegistry};
use delvewright_dsl::{DwCode, ExitTier};

/// `DW0470`: a hostile the party is *required* to kill can never be damaged.
pub const DW_UNDAMAGEABLE: DwCode = DwCode::every_version("DW0470", ExitTier::Build);

/// `DW0471`: a hostile the party is required to kill has no cell to be fought
/// from — its body is walled in.
pub const DW_UNREACHABLE: DwCode = DwCode::every_version("DW0471", ExitTier::Build);

/// `DW0472`: a mandatory encounter's declared health outlasts the best kit the
/// party can field, by the [`TTK_BUDGET_HITS`] sanity bound.
pub const DW_TTK_OVER_BUDGET: DwCode = DwCode::every_version("DW0472", ExitTier::Build);

/// `DW0473`: an unavoidable scripted hit on the critical path kills a
/// full-health player outright.
pub const DW_UNAVOIDABLE_LETHAL: DwCode = DwCode::every_version("DW0473", ExitTier::Build);

/// `DW0474`: a campaign with mandatory combat hands the party no sustain at all.
pub const DW_NO_SUSTAIN: DwCode = DwCode::every_version("DW0474", ExitTier::Build);

/// `DW0475`: (warning) the numeric time-to-kill bound could not be computed.
pub const DW_TTK_UNPROVEN: DwCode = DwCode::every_version("DW0475", ExitTier::Build);

/// `DW0477`: (warning) something the content bills `elite`/`boss` is one the
/// inverted floor gate cannot measure — so its silence in the run report means
/// "never fought", not "passed".
pub const DW_FLOOR_UNCOVERED: DwCode = DwCode::every_version("DW0477", ExitTier::Build);

/// The vanilla player's `minecraft:max_health` base value. The DSL exposes no
/// player-attribute surface at all, so this is not a default — it is the only
/// value a delve can ship.
///
/// One definition, in the metrics table (spec-0049 §2), which is also where the
/// survivable-fall figure derives from it.
pub use delvewright_dsl::metrics::PLAYER_MAX_HEALTH;

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

/// One mandatory encounter: a wave a `kill` objective on the compiled critical
/// path requires the party to clear.
#[derive(Debug, Clone)]
pub struct Encounter {
    /// The wave id (`wave/<kebab>`).
    pub wave_id: String,
    /// The `kill` objective the wave completes.
    pub objective_id: String,
    /// Index into `plan.critical_path` of the `kill` step — the compiler's OWN
    /// coordinate system, the one every `fire_step` and nav proof indexes.
    ///
    /// This is **not** the index the harness sees: the exported
    /// `critical-path.json` additionally carries a spliced `rest` step after the
    /// beat that arms each bonfire, so the two drift by one per bonfire.
    /// [`Plan::exported_step`] is the translation for that path, and the emitted
    /// combat plan states exported coordinates. (There is exactly one
    /// `combat-plan.json`, over the main path — spec-0025's per-branch paths
    /// resequence the same steps and need `emit::rest_step_index` instead, which
    /// is why this never crosses into them.)
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

/// Cheap gate: does this campaign have any mandatory **wave** fight? Used where
/// the question is genuinely about a `kill` step — the die-retry stage, the
/// checkpoint a death at a wave returns to.
///
/// **Not the test for "does this campaign have combat".** That is
/// [`mandatory_fights`]; see its doc comment for why the distinction cost the
/// island its whole spec-0023 pass.
pub fn has_encounters(plan: &Plan) -> bool {
    plan.critical_path
        .iter()
        .any(|s| matches!(s, Step::Kill { .. }))
}

/// Every fight the party cannot walk away from, of either shape.
///
/// ## The defect this replaces
///
/// The spec-0023 winnability pass — `DW0470` (a required hostile that cannot be
/// damaged), `DW0471` (nowhere to fight it from), `DW0472`/`DW0475` (time to
/// kill), `DW0473` (an unavoidable scripted one-shot), `DW0474` (the party
/// carries some sustain) — was gated on [`has_encounters`], which is
/// **`kill`-a-wave, the verb**. A campaign whose combat is *actors* therefore ran
/// none of it: `nobodys-cave-island` turns five bodies loose on the party, bills
/// one of them `elite`, ships zero `kill` objectives — and every one of those six
/// diagnostics was structurally unreachable on it for twenty-two owner rounds,
/// with `combat-plan.json` reporting `encounters: 0` and nothing anywhere saying
/// that was a coverage fact rather than a content fact (the staging gate's
/// `UNBOUND` verdict, row `bell-05`).
///
/// Hostility is [`hostile_actors`]'s predicate, not a second opinion: an
/// `unleash-actor` beat is the campaign's own statement that the party fights
/// this body, and nothing short of it can swing back.
#[derive(Clone, Debug, Default)]
pub struct Fights {
    /// Mandatory wave fights — one per critical-path `kill` step.
    pub waves: Vec<String>,
    /// Actors the campaign turns loose on the party.
    pub actors: Vec<String>,
}

impl Fights {
    /// The binding count: how many fights any combat proof has to reason about.
    pub fn total(&self) -> usize {
        self.waves.len() + self.actors.len()
    }

    /// Whether this campaign has combat at all.
    pub fn any(&self) -> bool {
        self.total() > 0
    }

    /// Why the count is zero, when it is — so a combat pass that examined nothing
    /// says so instead of returning a silent green.
    pub fn reason(&self) -> Option<&'static str> {
        (!self.any()).then_some(
            "this campaign declares no mandatory combat of either shape: no critical-path `kill` \
             objective names a wave, and no `unleash-actor` beat turns an actor loose on the \
             party. Every spec-0023 winnability proof is therefore inapplicable here rather than \
             passed — a delve with no fights is a legitimate state, and this is the line that \
             keeps it from reading as one that was checked",
        )
    }

    /// The `fights` block of `combat-plan.json`.
    pub fn to_json(&self) -> Value {
        let mut o = json!({
            "waves": self.waves,
            "actors": self.actors,
            "total": self.total(),
            "unbound": !self.any(),
        });
        if let Some(why) = self.reason() {
            o["reason"] = json!(why);
        }
        o
    }
}

/// Collect [`Fights`] for a compiled plan, in deterministic content order.
pub fn mandatory_fights(plan: &Plan) -> Fights {
    Fights {
        waves: encounters(plan).into_iter().map(|e| e.wave_id).collect(),
        actors: hostile_actors(plan.campaign)
            .into_iter()
            .map(|a| a.id.as_str().to_string())
            .collect(),
    }
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
        // campaign fires STRICTLY BEFORE the step (spec-0012 checkpoints are
        // party-wide and monotonic by quest order).
        //
        // `< i`, not `<= i`, and the difference is a real defect. A checkpoint's
        // `fire_step` is the step whose COMPLETION arms it — for a bonfire, the
        // beat after which a rest first becomes possible. A death *during* step
        // i happens while step i is unfinished, so a checkpoint armed by step i
        // does not exist yet at that death. `<= i` handed the encounter a
        // respawn point one beat in its own future: souls-bonfire arms bonfire 0
        // from `obj/slay`'s completion — the very kill this encounter IS — and
        // the plan claimed a death mid-fight would return the party to that
        // fire, when in truth it returns them to world spawn.
        //
        // Fixing it toward the STRICTER answer is deliberate: the die-retry
        // stage asserts the party respawns at the governing checkpoint, so an
        // over-generous claim here is a proof that measures the delve against a
        // rest point the player never had. An encounter with no governing
        // checkpoint reports none, and the ladder judges it for what it is.
        let checkpoint = plan
            .checkpoints
            .iter()
            .rfind(|cp| cp.fire_step < i)
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

// ---------------------------------------------------------------------------
// Tiered ACTORS — the other shape an elite takes (spec-0023 floor gate)
// ---------------------------------------------------------------------------

/// Whether the inverted floor gate can hold a billed encounter to its billing.
///
/// The whole point of naming this is that **silence must not read as a pass**.
/// Before actors carried a tier, an elite implemented as an actor was
/// structurally invisible to the gate: the run's finding list came back empty
/// and the ladder called that green while having fought nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FloorCoverage {
    /// The bot can engage it, so a first-try win is a real finding about the
    /// fight.
    Covered,
    /// It cannot be measured, and this is why. Carried verbatim into
    /// `combat-plan.json` and into [`DW_FLOOR_UNCOVERED`], so the run report
    /// says "not covered (reason)" instead of nothing at all.
    NotCovered(String),
}

impl FloorCoverage {
    /// Is this encounter one the gate actually measures?
    pub fn is_covered(&self) -> bool {
        matches!(self, FloorCoverage::Covered)
    }

    /// The reason it is not, if it is not.
    pub fn reason(&self) -> Option<&str> {
        match self {
            FloorCoverage::Covered => None,
            FloorCoverage::NotCovered(why) => Some(why),
        }
    }
}

/// One beat that stages or unleashes an actor: where it fires from, and — for a
/// trigger — what the player has to do to fire it.
///
/// This is what makes an actor fight *runnable* by the harness. A wave
/// encounter has a `kill` step on the critical path, so the bot already knows
/// how to start it; an actor fight starts because something got struck, used or
/// walked into, and that "something" is only stated here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorBeat {
    /// `trigger` / `quest` / `objective` / `trap` / `dialogue-respawn`.
    pub site: &'static str,
    /// The owning trigger / quest / trap id — or, for `dialogue-respawn`, the NPC
    /// whose tree hosts the option.
    pub owner: String,
    /// The objective, when the site is a quest's `on_objective_complete`.
    pub objective: Option<String>,
    /// JSON pointer to the effect itself, so a diagnostic can name it exactly.
    pub path: String,
    /// Trigger sites only: the event kind (`approach` / `strike` / `use` /
    /// `strike-npc`).
    pub on: Option<&'static str>,
    /// Trigger sites only: the anchor watched (absent for `strike-npc`, which
    /// watches a character rather than a place).
    pub at: Option<String>,
    /// `strike-npc` triggers only: the NPC whose body is the target.
    pub npc: Option<String>,
}

/// One tier-declaring stage-5 actor, as the validation ladder sees it.
#[derive(Clone, Debug)]
pub struct ActorEncounter {
    /// The actor id (`actor/<kebab>`).
    pub actor_id: String,
    /// Index into `quests.content.actors`, for the diagnostic's pointer.
    pub index: usize,
    /// The vanilla entity puppeted (and unleashed).
    pub entity: String,
    /// The custom name shown above it, if any.
    pub name: Option<String>,
    /// What the content bills the fight as.
    pub tier: EncounterTier,
    /// The anchor it is summoned on.
    pub anchor: String,
    /// That anchor resolved to a world cell (always `Some` past `DW0325`).
    pub pos: Option<[i32; 3]>,
    /// The body tag both the puppet and the unleashed twin wear.
    pub tag: String,
    /// Is the staged puppet damageable at all?
    pub vulnerable: bool,
    /// Declared attribute overrides — the body the party actually fights.
    pub attributes: Option<delvewright_dsl::MobAttributes>,
    /// Every beat that summons the puppet, in traversal order.
    pub spawned_by: Vec<ActorBeat>,
    /// Every beat that gives it real AI, in traversal order.
    pub unleashed_by: Vec<ActorBeat>,
    /// Whether the floor gate can measure this fight, and why not if it cannot.
    pub coverage: FloorCoverage,
}

/// Index every `spawn-actor` / `unleash-actor` beat in the campaign, by actor id.
///
/// Walks the one shared traversal ([`for_each_campaign_effect`]) rather than a
/// private one, so nesting — `sequence` steps, `on_arrive` reactions, flag-gated
/// bundles — is descended exactly as emission descends it, and an ambush (which
/// desugars to a real trigger at parse time) is seen as the trigger it becomes.
/// The dialogue **stage** is not exempt. `DialogueEffect` has no actor verb of
/// its own, but a dialogue option's `set-checkpoint` carries an `on_respawn`
/// bundle that is a `Vec<QuestEffect>`, so a `spawn-actor` there is a beat
/// emission lowers, and `EffectSite` must be able to *represent* it. The type is
/// wide enough for the walk to be wide; that is what carried roots 6 and 7
/// (spec-0031) in on the day they were added, and it is the property the
/// exhaustive match below exists to keep.
fn actor_beats(c: &Campaign) -> BTreeMap<String, (Vec<ActorBeat>, Vec<ActorBeat>)> {
    let triggers: BTreeMap<&str, &delvewright_dsl::EnvTrigger> = c
        .quests
        .content
        .triggers
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();
    let mut out: BTreeMap<String, (Vec<ActorBeat>, Vec<ActorBeat>)> = BTreeMap::new();
    for_each_campaign_effect(c, &mut |path, site, eff| {
        let (actor, unleash) = match eff {
            QuestEffect::SpawnActor { actor, .. } => (actor, false),
            QuestEffect::UnleashActor { actor, .. } => (actor, true),
            _ => return,
        };
        let (kind, owner, objective) = match site {
            EffectSite::Objective { quest, objective } => {
                ("objective", quest.clone(), Some(objective.clone()))
            }
            EffectSite::QuestComplete { quest } => ("quest", quest.clone(), None),
            EffectSite::Trigger { trigger } => ("trigger", trigger.clone(), None),
            EffectSite::Trap { trap } => ("trap", trap.clone(), None),
            // Effect root 5. A `spawn-actor`/`unleash-actor` nested in a dialogue
            // option's `set-checkpoint` `on_respawn` bundle is lowered into
            // `cp_on_respawn_<i>` and really does put a body in the world, so it is
            // an actor beat like any other. It is ambient — re-run on death while
            // that checkpoint is active — so, like a trigger and a trap, it has no
            // DAG position.
            EffectSite::DialogueRespawn { npc, .. } => ("dialogue-respawn", npc.clone(), None),
            // Effect roots 6 and 7 (spec-0031). Both are ambient for the same
            // reason the two above are: the party may earn the shortcut at any
            // time or never, and nobody is forced to die. A body put in the world
            // from either is a beat the floor gate must be able to name.
            EffectSite::ShortcutUnlock { shortcut } => ("shortcut-unlock", shortcut.clone(), None),
            EffectSite::OnDeath => ("on-death", "on_death".to_string(), None),
            // Effect root 8 (spec-0032). Ambient like the four above: a shop
            // offer fires when a player presses a button, which they may do at
            // any time or never.
            EffectSite::ShopOffer { shop, .. } => ("shop-offer", shop.clone(), None),
        };
        let t = (kind == "trigger")
            .then(|| triggers.get(owner.as_str()))
            .flatten();
        let beat = ActorBeat {
            site: kind,
            owner,
            objective,
            path: path.to_string(),
            on: t.map(|t| t.on.kind()),
            at: t
                .and_then(|t| t.at.as_ref())
                .map(|a| a.as_str().to_string()),
            npc: t
                .and_then(|t| t.on.npc_target())
                .map(|n| n.as_str().to_string()),
        };
        let slot = out.entry(actor.as_str().to_string()).or_default();
        if unleash {
            slot.1.push(beat);
        } else {
            slot.0.push(beat);
        }
    });
    out
}

/// Can the unassisted bot be made to fight this actor at all — and if not, the
/// one sentence that says why, in the words the author needs to fix it.
///
/// The rule is **unleash or nothing**, and that is a judgement worth stating.
/// An `unleash-actor` beat replaces the puppet with a real-AI twin of the same
/// body, and that twin is always killable (its summon carries no `Invulnerable`
/// whatever the actor's `vulnerable` flag says — the same fact `DW0470` records).
/// Everything short of that is not a fight:
///
/// - never summoned → the puppet never exists;
/// - summoned but not `vulnerable` → it is `Invulnerable` scenery;
/// - summoned and `vulnerable` but never unleashed → damageable, but `NoAI` and
///   knockback-immune, so it never swings back. A target that cannot fight back
///   is beaten cold by construction, and a floor warning derived from that would
///   be an artifact of the check rather than a finding about the encounter.
fn actor_coverage(a: &Actor, spawns: &[ActorBeat], unleashes: &[ActorBeat]) -> FloorCoverage {
    if !unleashes.is_empty() {
        return FloorCoverage::Covered;
    }
    let id = a.id.as_str();
    if spawns.is_empty() {
        return FloorCoverage::NotCovered(format!(
            "no `spawn-actor` effect anywhere in the campaign summons `{id}`, so the puppet never \
             exists and there is nothing for the bot to fight"
        ));
    }
    if a.vulnerable {
        FloorCoverage::NotCovered(format!(
            "`{id}` is only ever staged as a `vulnerable` puppet: damageable, but `NoAI` and \
             knockback-immune, so it never attacks. Anything that cannot fight back is beaten \
             cold by construction, so a floor finding derived from it would say nothing about \
             the encounter. Add an `unleash-actor` beat to make it a fight the gate can measure"
        ))
    } else {
        FloorCoverage::NotCovered(format!(
            "`{id}` is staged but never unleashed, and it is not `vulnerable` — the puppet is \
             summoned `Invulnerable`, so it is scenery the party walks past, not a fight. Add an \
             `unleash-actor` beat (or drop the tier)"
        ))
    }
}

/// Every tier-declaring actor, in declaration order, with its staging beats and
/// its floor-gate coverage resolved.
///
/// Empty for every campaign that declares no actor `tier` — which is every
/// campaign written before this field existed, so nothing an existing delve
/// emits moves.
pub fn actor_encounters(plan: &Plan) -> Vec<ActorEncounter> {
    let c = plan.campaign;
    let beats = actor_beats(c);
    let mut out = Vec::new();
    for (index, a) in c.quests.content.actors.iter().enumerate() {
        let Some(tier) = a.tier else { continue };
        let (spawns, unleashes) = beats.get(a.id.as_str()).cloned().unwrap_or_default();
        let coverage = actor_coverage(a, &spawns, &unleashes);
        out.push(ActorEncounter {
            actor_id: a.id.as_str().to_string(),
            index,
            entity: a.entity.clone(),
            name: a.name.clone(),
            tier,
            anchor: a.anchor.as_str().to_string(),
            pos: plan.point_any(a.anchor.as_str()),
            tag: format!("dw_actor_{}", safe_local(a.id.as_str())),
            vulnerable: a.vulnerable,
            attributes: a.attributes,
            spawned_by: spawns,
            unleashed_by: unleashes,
            coverage,
        });
    }
    out
}

/// A tier-declaring wave that no critical-path `kill` step names.
///
/// The same silence, on the shape that already had a `tier`: `encounters()`
/// collects only the MANDATORY waves, so an optional wave billed `elite` was as
/// invisible to the floor gate as a tiered actor was. Found here rather than in
/// a separate pass because it is one question — "what does the gate cover?" —
/// and one question deserves one answer.
fn uncovered_tiered_waves<'a>(plan: &Plan<'a>) -> Vec<(usize, &'a Wave, String)> {
    let mandatory = mandatory_waves(plan);
    plan.campaign
        .quests
        .content
        .waves
        .iter()
        .enumerate()
        .filter(|(_, w)| w.tier.is_some_and(EncounterTier::has_floor_expectation))
        .filter(|(_, w)| !mandatory.contains(w.id.as_str()))
        .map(|(i, w)| {
            (
                i,
                w,
                format!(
                    "no `kill` objective on the compiled critical path names `{}`, so the bot \
                     never fights it — a tier on an optional wave is a claim nothing measures. \
                     Give the wave a `kill` objective on the path, or drop the tier",
                    w.id.as_str()
                ),
            )
        })
        .collect()
}

/// Every actor the campaign turns loose on the party, in declaration order —
/// the campaign's own answer to "which actors are *fights*".
///
/// Hostility is read off the campaign's own declarations, by the SAME rule the
/// die-retry / assist machinery uses to decide an actor is a fight at all
/// ([`actor_coverage`]'s "unleash or nothing"): an `unleash-actor` beat replaces
/// the puppet with a real-AI twin that swings back, and nothing short of it can
/// damage a player — a staged puppet is `NoAI` and knockback-immune (and
/// `Invulnerable` unless `vulnerable`), so it never attacks. Never inferred from
/// the species: the pinned entity registry is a membership set with no
/// mob-category data (`DW0469`'s rule), so the compiler cannot and does not ask
/// whether `minecraft:sheep` is a monster.
///
/// One predicate, two consumers: the floor-gate ledger below, and the bonfire's
/// undefeated re-seat (spec-0016 §1) — which must refresh exactly the bodies the
/// party can be fighting when they rest, and nothing that is scenery.
pub fn hostile_actors(c: &Campaign) -> Vec<&Actor> {
    let beats = actor_beats(c);
    c.quests
        .content
        .actors
        .iter()
        .filter(|a| {
            beats
                .get(a.id.as_str())
                .is_some_and(|(_, unleashes)| !unleashes.is_empty())
        })
        .collect()
}

/// Every actor the campaign turns loose on the party but never bills:
/// `unleash-actor`ed somewhere, `tier` absent.
///
/// A tier declared `ordinary` is a *statement* — the author saying this fight is
/// routine — and stays off the ledger like any other ordinary encounter. An
/// ABSENT tier is not a statement, and that is the whole difference this
/// function exists to keep.
fn untiered_hostile_actors(c: &Campaign) -> Vec<&Actor> {
    hostile_actors(c)
        .into_iter()
        .filter(|a| a.tier.is_none())
        .collect()
}

/// Does this campaign turn any unbilled actor loose on the party? Emission asks,
/// because a campaign whose only hostile is an untiered actor must still ship a
/// ledger that says so — see [`untiered_hostile_actors`].
pub fn has_untiered_hostile_actors(plan: &Plan) -> bool {
    !untiered_hostile_actors(plan.campaign).is_empty()
}

/// One line of the floor-gate ledger: what the content declares, and whether the
/// gate can hold it to that.
struct FloorEntry {
    kind: &'static str,
    id: String,
    /// The declared tier, or `None` for a hostile that declared none — which is
    /// exactly why it is on the ledger.
    tier: Option<EncounterTier>,
    coverage: FloorCoverage,
}

/// The whole floor-gate ledger for a campaign, covered and uncovered together,
/// in a fixed order: mandatory waves in critical-path order, then optional
/// tiered waves in declaration order, then tiered actors in declaration order,
/// then untiered hostile actors in declaration order.
///
/// The last group is why an EMPTY ledger cannot be trusted to mean "everything
/// is covered": without it, an actor the campaign unleashes on the party
/// without declaring a tier appears on neither side of the ledger, and the run
/// report prints two empty lists over a delve full of fights. That reads as
/// "everything is covered" when it means "nothing was even assessed".
/// Silence must not read as a pass — and an
/// unassessed fight is silence of exactly the kind [`FloorCoverage`] exists to
/// break.
fn floor_ledger(
    plan: &Plan,
    mandatory: &[Encounter],
    actors: &[ActorEncounter],
) -> Vec<FloorEntry> {
    let mut out: Vec<FloorEntry> = mandatory
        .iter()
        .filter(|e| e.tier.has_floor_expectation())
        .map(|e| FloorEntry {
            kind: "wave",
            id: e.wave_id.clone(),
            tier: Some(e.tier),
            coverage: FloorCoverage::Covered,
        })
        .collect();
    for (_, w, why) in uncovered_tiered_waves(plan) {
        out.push(FloorEntry {
            kind: "wave",
            id: w.id.as_str().to_string(),
            tier: Some(w.tier.unwrap_or_default()),
            coverage: FloorCoverage::NotCovered(why),
        });
    }
    for a in actors.iter().filter(|a| a.tier.has_floor_expectation()) {
        out.push(FloorEntry {
            kind: "actor",
            id: a.actor_id.clone(),
            tier: Some(a.tier),
            coverage: a.coverage.clone(),
        });
    }
    for a in untiered_hostile_actors(plan.campaign) {
        let id = a.id.as_str();
        out.push(FloorEntry {
            kind: "actor",
            id: id.to_string(),
            tier: None,
            coverage: FloorCoverage::NotCovered(format!(
                "`{id}` is UNTIERED: the campaign `unleash-actor`s it, so the party fights a \
                 real-AI body that swings back, but nothing declares what that fight is worth — \
                 so the inverted floor gate never assessed it at all. An untiered hostile is not \
                 a covered fight and its absence from the findings is not a pass. Declare a \
                 `tier`: `ordinary` if the fight is meant to be routine (which takes it off this \
                 ledger as a statement rather than an omission), `elite`/`boss` if it is billed \
                 hard and should be measured"
            )),
        });
    }
    out
}

/// `DW0477` — one warning per billed encounter the floor gate cannot measure.
///
/// Warning tier, one diagnostic per finding with its exact JSON pointer: an
/// unmeasurable elite is a real gap in the verification, but it is a *design*
/// statement (the author may genuinely want an `Invulnerable` set-dressing giant
/// they also called a boss), and spec-0023 puts the floor gate itself at
/// advisory tier. What is not negotiable is that it be said out loud.
pub fn floor_coverage_warnings(
    plan: &Plan,
    mandatory: &[Encounter],
    actors: &[ActorEncounter],
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let index_of: BTreeMap<&str, usize> = actors
        .iter()
        .map(|a| (a.actor_id.as_str(), a.index))
        .collect();
    let uncovered_wave_index: BTreeMap<String, usize> = uncovered_tiered_waves(plan)
        .into_iter()
        .map(|(i, w, _)| (w.id.as_str().to_string(), i))
        .collect();
    for e in floor_ledger(plan, mandatory, actors) {
        let Some(why) = e.coverage.reason() else {
            continue;
        };
        // An UNTIERED hostile is on the ledger but is not BILLED
        // anything, and `DW0477` is by definition about a billing the gate
        // cannot hold — its message, its pointer (`…/tier`, a field that does
        // not exist here) and its prescription would all be wrong. The ledger
        // line, which the run report prints verbatim, is the whole record.
        let Some(tier) = e.tier else {
            continue;
        };
        let path = match e.kind {
            "actor" => format!("/content/actors/{}/tier", index_of[e.id.as_str()]),
            _ => format!("/content/waves/{}/tier", uncovered_wave_index[&e.id]),
        };
        out.push(Diagnostic::warning(
            DW_FLOOR_UNCOVERED,
            "quests",
            path,
            format!(
                "`{}` is billed `{}`, but the validation ladder's inverted floor gate cannot \
                 measure it: {why}.\n\nThis matters because of how the gate reports: it emits a \
                 warning when the UNASSISTED bot beats a billed elite on its first attempt, and \
                 says nothing otherwise — so an encounter the bot never fought produces exactly \
                 the same silence as one it fought and lost. `validation/combat-plan.json` \
                 records this fight as `floor-gate: not covered`, with this reason, so the run \
                 report cannot present the silence as a pass. Warning tier because an \
                 unmeasurable elite is a legitimate design (set dressing the content also chose \
                 to name) — what is not legitimate is nobody knowing.",
                e.id,
                tier.token()
            ),
        ));
    }
    out
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
) -> Result<Vec<Diagnostic>, Failure> {
    let c = plan.campaign;
    let items = ItemCombatRegistry::v1_21_11();
    let types = DamageTypeRegistry::v1_21_11();
    let difficulty = effective_difficulty(c);
    let mandatory = mandatory_waves(plan);
    let mut warnings = Vec::new();

    // ---- DW0470: every required hostile can be hurt at all -----------------
    let undamageable = undamageable_hostiles(&c.quests.content.waves, &mandatory);
    if !undamageable.is_empty() {
        return Err(Failure {
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
        return Err(Failure {
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
                    return Err(Failure {
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
        return Err(Failure {
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
    //
    // Over EVERY fight, not just the wave-shaped ones. "Does the party need food"
    // is a question about how much fighting they have to do, and an actor the
    // campaign unleashes on them is a fight by the campaign's own declaration —
    // keying this to `encounters` alone made a delve whose combat is entirely
    // actors structurally unable to raise it. See [`mandatory_fights`].
    let fights = mandatory_fights(plan);
    if fights.any() && !has_any_sustain(c, &items) {
        warnings.push(Diagnostic::warning(
            DW_NO_SUSTAIN,
            "classes",
            "/content/classes",
            format!(
                "this campaign has {total} mandatory fight(s) — {waves} wave encounter(s) and \
                 {actors} actor(s) it turns loose on the party — and hands them no sustain at \
                 all: no class kit, `give-item` effect or `loot` container anywhere carries an \
                 item with a `minecraft:food` component. Natural regeneration stops the moment \
                 the hunger bar drops below 18, so after the first fight the party's health only \
                 ever goes down. Fix: put food in the kits, or stock a container on the route. \
                 Warning tier because the fight budget a party actually needs depends on play \
                 the compiler is forbidden to model (spec-0023 \"Out of scope\") — the finding \
                 here is the literal zero, which is a design fact rather than a balance opinion.",
                total = fights.total(),
                waves = fights.waves.len(),
                actors = fights.actors.len(),
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

// ---------------------------------------------------------------------------
// When to stop swinging at one body (the per-encounter half)
// ---------------------------------------------------------------------------

/// How many times over the arithmetic's fully-charged swing count a single body
/// may be meleed before the validation ladder stops swinging at it and says so.
///
/// **Authored, not cited.** Vanilla scales a swing's damage by the attack-cooldown
/// progress, and the ladder's bot swings on a fixed cadence without ever waiting
/// the cooldown out, so it lands well under full damage every time and needs
/// several times the count this arithmetic produces for the same body. No source
/// gives the right multiple for a bot's fencing, so this is a sanity margin in
/// exactly the spirit of [`TTK_BUDGET_HITS`]: deliberately generous, crossed only
/// by a body that is not dying at all rather than by one that is merely tanky.
///
/// It lives here, beside the arithmetic, and not in the harness, because the
/// number it multiplies is a fact about the ENCOUNTER — a harness constant would
/// be the same figure for a hall of rats and for a boss.
pub const GIVE_UP_SWING_MARGIN: u32 = 8;

/// The smallest melee budget any body gets, whatever the arithmetic says.
///
/// **Authored, not cited**, and for one reason: a mob the best kit fells in a
/// single fully-charged swing would otherwise get eight, which a bot that misses
/// twice while a mob backs away can spend without the body being unkillable at
/// all. The floor keeps the budget a statement about "this body is not dying"
/// rather than about the bot's aim.
pub const GIVE_UP_SWING_FLOOR: u32 = 16;

/// One kind of body standing at an encounter, with the melee budget the
/// encounter's own arithmetic gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyBound {
    /// Entity kind in the client's vocabulary (`zombie`), which is the only
    /// identity the bot can read off a body.
    pub kind: String,
    /// How many of them the wave seats.
    pub count: u32,
    /// Swings after which this body is not being killed by the class kit —
    /// `None` when the arithmetic could not be computed.
    pub give_up_swings: Option<u32>,
    /// Why there is no budget. Present exactly when `give_up_swings` is `None`.
    pub reason: Option<String>,
}

/// The melee budgets for one encounter's bodies, worst-case per kind.
///
/// # What this replaces
///
/// The bot used to decide a body was unkillable by meleeing it for a fixed six
/// seconds — a combat-targeting policy with no author, written in the harness,
/// reasoned about in the comments of one campaign ("a `minecraft:warden` posing
/// as Polyphemus"). Six seconds is not a fact about anything: it is too long for
/// a rat and too short for an elite, and when it fires it silently blacklists the
/// body and reports nothing.
///
/// The encounter already knows better. `attributes.max_health`, the mob's
/// resistance, and the best weapon any class kit carries are the same three
/// numbers [`check_winnability`] bounds the whole fight with; per body they give
/// the swings that body should take. A body that outlives them is a **content
/// defect the run report names** — not a blacklist the harness invents.
///
/// # Grouping
///
/// A wave may seat two stacks of one entity with different tuning, and the bot
/// cannot tell them apart (it reads a name, not NBT). So kinds are grouped and
/// the budget is the WORST of the stacks — and a single unproven stack makes the
/// whole kind unproven. Never the other direction: giving up early on a body that
/// was merely tougher than the stack beside it would fail a delve that is fine.
pub fn encounter_bodies(c: &Campaign, wave: &Wave, items: &ItemCombatRegistry) -> Vec<BodyBound> {
    let best = best_melee_hit(c, items);
    // kind -> (count, worst swings so far, first reason it could not be bounded)
    let mut by_kind: BTreeMap<&str, (u32, Option<u32>, Option<String>)> = BTreeMap::new();
    for mob in &wave.mobs {
        let kind = client_name(&mob.entity);
        let entry = by_kind.entry(kind).or_insert((0, Some(0), None));
        entry.0 += mob.count;
        let bound = body_swings(mob, best.as_ref());
        match bound {
            Ok(swings) => {
                if let Some(worst) = entry.1 {
                    entry.1 = Some(worst.max(swings));
                }
            }
            Err(why) => {
                entry.1 = None;
                if entry.2.is_none() {
                    entry.2 = Some(why);
                }
            }
        }
    }
    by_kind
        .into_iter()
        .map(|(kind, (count, swings, reason))| BodyBound {
            kind: kind.to_string(),
            count,
            give_up_swings: swings,
            reason: swings.is_none().then(|| {
                reason.unwrap_or_else(|| "the arithmetic could not be computed".to_string())
            }),
        })
        .collect()
}

/// The melee budget for ONE body of `mob`, or why there is none.
fn body_swings(mob: &WaveMob, best: Option<&(f64, String, String)>) -> Result<u32, String> {
    let Some((hit, class, item)) = best else {
        return Err(
            "no class kit carries an item with an `attack_damage` attribute, so the party's \
             damage output is unknown (a bow's damage is projectile code and appears in no \
             vanilla data — absence is not zero). Nothing here can say how long a body should \
             take to fall."
                .to_string(),
        );
    };
    let Some(max_health) = mob.attributes.and_then(|a| a.max_health) else {
        return Err(format!(
            "{} declares no `attributes.max_health`, and Mojang publishes no per-entity default \
             attributes — so its health is genuinely unknown at build time and this compiler \
             refuses to invent a health table. Declare `attributes.max_health` on the stack to \
             give the ladder a melee budget for this body (the same declaration `DW0475` asks \
             for).",
            mob.entity
        ));
    };
    let (multiplier, _) = mob_damage_multiplier(mob);
    // multiplier 0 (total immunity) is `DW0470`, refused long before here.
    let effective = max_health / multiplier;
    let charged = (effective / hit).ceil().max(1.0) as u64;
    let budget = charged
        .saturating_mul(u64::from(GIVE_UP_SWING_MARGIN))
        .max(u64::from(GIVE_UP_SWING_FLOOR));
    debug_assert!(
        !class.is_empty() && !item.is_empty(),
        "best_melee_hit names the kit it came from"
    );
    Ok(u32::try_from(budget).unwrap_or(u32::MAX))
}

/// `bodies` for one encounter, in the plan's vocabulary.
fn bodies_json(bodies: &[BodyBound]) -> Value {
    Value::Array(
        bodies
            .iter()
            .map(|b| {
                let mut o = json!({
                    "kind": b.kind,
                    "count": b.count,
                    "give_up_swings": b.give_up_swings,
                });
                if let Some(why) = &b.reason {
                    o["reason"] = json!(why);
                }
                o
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Who is not a fight (the cast half of the targeting policy)
// ---------------------------------------------------------------------------

/// Strip the namespace off an entity id, giving the name a client reports.
///
/// The validation bot classifies a body by `entity.name` — `"villager"`,
/// `"mannequin"` — because that is the only identity mineflayer exposes on
/// 1.21.11 (entity `Tags` are not readable from the client). So every entity id
/// this module hands the harness is stated in that vocabulary, once, here.
fn client_name(entity: &str) -> &str {
    entity.rsplit(':').next().unwrap_or(entity)
}

/// The delve's cast statement: which entity kinds are never a combat target.
///
/// # Why the compiler owns this
///
/// The bot cannot read entity tags, so it decides what to swing at by SHAPE, and
/// a shape test cannot tell a quest-giver from a zombie when the quest-giver *is*
/// a zombie. The harness used to carry the answer as a literal set containing
/// `mannequin` and `villager` — which is not a fact about Minecraft, it is a fact
/// about what THIS compiler summons an NPC as, written down in the one place that
/// cannot know it. An author who gives an NPC `base_entity: minecraft:zombie` gets
/// a quest-giver the bot beats to death, and nothing anywhere would have said so.
///
/// Every NPC body the compiler emits is `Invulnerable:1b` — both the skinned
/// `minecraft:mannequin` branch and the plain `base_entity` branch — so an NPC is
/// never a fight, whatever it is wearing. That is the whole rule.
///
/// # The collision, and why it is stated rather than resolved
///
/// A kind can be an NPC body *and* a body the party fights: an NPC villager in a
/// campaign whose wave is villagers. Excluding the kind would make the wave
/// unkillable; not excluding it leaves the NPC attackable. The compiler resolves
/// it in the only direction that cannot soft-lock a delve — the fightable kind
/// wins — and says so in [`NonCombatants::ambiguous`], so the run report names
/// the NPCs the bot may swing at instead of the harness silently deciding.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NonCombatants {
    /// Entity kinds (client vocabulary) no body of which is ever a fight.
    pub kinds: Vec<String>,
    /// Kinds that are an NPC body *and* a fightable body — excluded from
    /// [`Self::kinds`] with the reason.
    pub ambiguous: Vec<(String, String)>,
    /// How many NPC bodies were examined — the binding count.
    pub examined: usize,
}

/// `non_combatants`'s reason when `examined == 0`: the campaign has no NPCs at
/// all, so there is no body the bot must be told to leave alone.
const NON_COMBATANTS_UNBOUND_REASON: &str = "this campaign stages no NPC, so no body in it is \
    exempt from combat by cast. Stated explicitly so an empty `kinds` list is never read as \
    \"the compiler forgot\": every mob-shaped body in this delve is either a wave mob, an \
    actor, or vanilla furniture the bot classifies by shape.";

/// Census the cast: what the bot must never swing at, and what it may.
///
/// Deterministic: both lists are built through a `BTreeSet`, never insertion
/// order.
pub fn non_combatants(c: &Campaign) -> NonCombatants {
    // Every kind the party is meant to be able to fight. An actor's `entity` is
    // here whether or not the puppet is currently `Invulnerable`: unleashing
    // replaces the puppet with a real-AI twin summoned as that same entity and
    // carrying no `Invulnerable` flag, so the kind is fightable and only the
    // moment differs.
    let mut fightable: BTreeSet<&str> = BTreeSet::new();
    for w in &c.quests.content.waves {
        for m in &w.mobs {
            fightable.insert(client_name(&m.entity));
        }
    }
    for a in &c.quests.content.actors {
        fightable.insert(client_name(&a.entity));
    }

    let mut kinds: BTreeSet<&str> = BTreeSet::new();
    let mut ambiguous: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut examined = 0usize;
    for npc in &c.npcs.content.npcs {
        examined += 1;
        // The skinned branch re-dresses the NPC as a `minecraft:mannequin`; the
        // plain branch summons `base_entity`. One rule, both branches, and it is
        // the emitter's rule (`emit::npc_body_commands`), not a second opinion.
        let body = if npc.skin.is_some() {
            "mannequin"
        } else {
            client_name(&npc.base_entity)
        };
        if fightable.contains(body) {
            ambiguous.entry(body).or_default().insert(npc.id.as_str());
        } else {
            kinds.insert(body);
        }
    }

    NonCombatants {
        kinds: kinds.into_iter().map(str::to_string).collect(),
        ambiguous: ambiguous
            .into_iter()
            .map(|(kind, npcs)| {
                (
                    kind.to_string(),
                    format!(
                        "{} is also a wave mob or an actor in this campaign, so it cannot be \
                         excluded from targeting without making that fight unwinnable. The bot \
                         may therefore swing at {} — move the NPC onto a `base_entity` nothing \
                         fights, or give it a `skin` (which embodies it as a \
                         `minecraft:mannequin`).",
                        kind,
                        npcs.iter().copied().collect::<Vec<_>>().join(", ")
                    ),
                )
            })
            .collect(),
        examined,
    }
}

/// The `non_combatants` block of `critical-path.json`.
pub fn non_combatants_json(c: &Campaign) -> Value {
    let nc = non_combatants(c);
    let mut o = json!({
        "kinds": nc.kinds,
        "ambiguous": nc.ambiguous.iter()
            .map(|(kind, why)| json!({ "kind": kind, "why": why }))
            .collect::<Vec<_>>(),
        // playtest-methodology.md rule 1: the binding count, stated in the
        // artifact, so a reader never has to notice an empty list to learn this
        // census matched nothing.
        "examined": nc.examined,
        "unbound": nc.examined == 0,
    });
    if nc.examined == 0 {
        o["reason"] = json!(NON_COMBATANTS_UNBOUND_REASON);
    }
    o
}

/// One staging beat, as the plan states it.
fn beat_json(b: &ActorBeat) -> Value {
    let mut o = json!({ "site": b.site, "owner": b.owner, "path": b.path });
    if let Some(objective) = &b.objective {
        o["objective"] = json!(objective);
    }
    if let Some(on) = b.on {
        o["on"] = json!(on);
    }
    if let Some(at) = &b.at {
        o["at"] = json!(at);
    }
    if let Some(npc) = &b.npc {
        o["npc"] = json!(npc);
    }
    o
}

/// One tiered actor, as the plan states it.
fn actor_json(a: &ActorEncounter) -> Value {
    let mut o = json!({
        "actor": a.actor_id,
        "entity": a.entity,
        "tier": a.tier.token(),
        "anchor": a.anchor,
        "tag": a.tag,
        "vulnerable": a.vulnerable,
        "spawned_by": a.spawned_by.iter().map(beat_json).collect::<Vec<_>>(),
        "unleashed_by": a.unleashed_by.iter().map(beat_json).collect::<Vec<_>>(),
        "floor_gate": coverage_json(&a.coverage),
    });
    if let Some(name) = &a.name {
        // spec-0029 named exclusion: `combat-plan.json` is the validation ladder's
        // own artifact, read by the bot and by a maintainer, never rendered to a
        // player — so the actor's name appears here as its English source, not as
        // a translate key. (The name became translatable when `actors[].name`
        // entered the l10n inventory; before that this line could not have carried
        // a tag at all.)
        o["name"] = json!(delvewright_dsl::l10n_plain(name));
    }
    if let Some(pos) = a.pos {
        o["pos"] = json!([pos[0], pos[1], pos[2]]);
    }
    if let Some(attrs) = a.attributes {
        o["attributes"] = serde_json::to_value(attrs).expect("MobAttributes serializes");
    }
    o
}

/// Coverage, spelled so that a reader who skips the prose still cannot mistake
/// "not covered" for "passed".
fn coverage_json(c: &FloorCoverage) -> Value {
    match c {
        FloorCoverage::Covered => json!({ "covered": true }),
        FloorCoverage::NotCovered(why) => json!({ "covered": false, "reason": why }),
    }
}

/// The validation-only combat plan the bot ladder reads (spec-0023 §1/§3/§4).
///
/// Lives under `validation/` like the waypoint export — excluded from the
/// shipped delve image, so declaring an encounter tier or running the die-retry
/// stage can never change a shipped byte. Emitted when the campaign has a
/// mandatory encounter, a tier-declaring actor **or** an untiered hostile actor,
/// so a combat-free delve's output is unchanged entirely — and a delve whose
/// only fight is an unbilled actor still ships the ledger that says so.
///
/// Three arrays, deliberately separate:
///
/// * `encounters` — the mandatory wave fights, unchanged in shape. These are the
///   ones the die-retry stage runs; nothing else may be poured into this array,
///   because "there is a checkpoint a death here returns to" is a property only
///   a critical-path `kill` step has.
/// * `actors` — the tier-declaring stage-5 actors, with the anchor to walk to,
///   the tag the body wears, the beats that stage and unleash them, and the
///   attributes of the body that fights.
/// * `floor_gate` — the ledger: every encounter billed `elite`/`boss` **plus
///   every untiered hostile actor**, split into what the gate covers
///   and what it cannot, each uncovered entry naming its reason. An untiered
///   hostile carries `tier: null` and lands in `not_covered`, because a fight
///   nothing bills is a fight nothing assessed. This exists so an empty findings
///   list can never be read as a pass over encounters that were never fought.
///
/// # Binding counts (playtest-methodology.md rule 1)
///
/// A ledger that MATCHED zero objects and a ledger that matched several and
/// found them all fine are indistinguishable to a reader who is not counting —
/// the island shipped exactly that silence for nineteen rounds, on
/// `floor_gate.covered`/`not_covered` **and** `actors[]` all empty at once,
/// before round 20 declared the campaign's first actor `tier`. This is
/// reporting, not diagnosis: an empty ledger is often the honest answer (an
/// all-`ordinary` delve binds nothing, on purpose), so nothing here fails the
/// build or mints a new DW code. What it does is say the binding count OUT
/// LOUD, additively, on both surfaces that can go quietly empty:
///
/// * `floor_gate.examined` / `.unbound` / `.reason` — `examined` is
///   `covered.len() + not_covered.len()`; `unbound` is `examined == 0`, with a
///   `reason` present exactly then. Note this can be `unbound` even when
///   `actors[]` is not: an actor declared `tier: "ordinary"` populates
///   `actors[]` (any declared tier does) but never the floor ledger (only
///   `elite`/`boss` carries a floor expectation), so an all-`ordinary` campaign
///   is `actors[].examined > 0` and `floor_gate.unbound` at once — two
///   different questions, two different counts.
/// * a sibling `actors_gate` (`examined`/`unbound`/`reason`) states the same
///   for `actors[]` itself: how many actors this build's tier machinery even
///   tracked. `actors[]` holds every TIER-DECLARING actor, whether the floor
///   gate covers it or not — an untiered hostile actor never appears here (it
///   is `floor_gate.not_covered` only), so `actors_gate.unbound`
///   does not by itself mean "no hostile actor in this campaign"; the reason
///   text says so and points at `floor_gate.not_covered`.
pub fn combat_plan_json(plan: &Plan, encounters: &[Encounter], actors: &[ActorEncounter]) -> Value {
    let difficulty = effective_difficulty(plan.campaign);
    let items = ItemCombatRegistry::v1_21_11();
    let entries: Vec<Value> = encounters
        .iter()
        .map(|e| {
            // What stands at this encounter, and how long each body should take
            // to fall. The bot picks its target by the name a client reports, so
            // this is the encounter stating its own cast in that vocabulary —
            // see `encounter_bodies` for what it replaces.
            let bodies = plan::wave_of(plan.campaign, &e.wave_id)
                .map(|w| encounter_bodies(plan.campaign, w, &items))
                .unwrap_or_default();
            let mut o = json!({
                "wave": e.wave_id,
                "objective": e.objective_id,
                // EXPORTED coordinates — the index this encounter's `kill` step
                // has in `critical-path.json`, rest splices included. The plan
                // is a harness document, and every harness document speaks one
                // coordinate system.
                "step": plan.exported_step(e.step),
                "tier": e.tier.token(),
                "pos": [e.pos[0], e.pos[1], e.pos[2]],
                "count": e.count,
                "respawns_on_rest": e.respawns_on_rest,
                // The encounter's own cast and melee budgets. `bodies[].kind` is
                // what the bot MAY swing at here; `give_up_swings` is when it
                // must stop and let the report name the body.
                "bodies": bodies_json(&bodies),
                // The tag-census probe surface for this wave. The
                // harness calls what the plan NAMES — `safe_local` is a compiler
                // naming rule, and a harness that re-derived it would be exactly
                // the downstream folklore CLAUDE.md forbids.
                "census": {
                    "census": format!("{ns}:wave_census_{safe}", ns = plan.namespace,
                                      safe = crate::plan::safe_local(&e.wave_id)),
                    "brand": format!("{ns}:wave_brand_{safe}", ns = plan.namespace,
                                     safe = crate::plan::safe_local(&e.wave_id)),
                    "unbrand": format!("{ns}:wave_unbrand_{safe}", ns = plan.namespace,
                                       safe = crate::plan::safe_local(&e.wave_id)),
                },
            });
            if let Some(cp) = e.checkpoint {
                o["checkpoint"] = json!([cp[0], cp[1], cp[2]]);
            }
            o
        })
        .collect();
    let ledger = floor_ledger(plan, encounters, actors);
    let (covered, not_covered): (Vec<&FloorEntry>, Vec<&FloorEntry>) =
        ledger.iter().partition(|e| e.coverage.is_covered());
    let floor_examined = covered.len() + not_covered.len();
    let mut floor_gate = json!({
        "covered": covered
            .iter()
            .map(|e| json!({
                "kind": e.kind,
                "id": e.id,
                "tier": e.tier.map(EncounterTier::token),
            }))
            .collect::<Vec<_>>(),
        // `tier: null` is the untiered hostile — an explicit
        // null rather than an omitted key, because this document's entire
        // job is to make an absence legible.
        "not_covered": not_covered
            .iter()
            .map(|e| json!({
                "kind": e.kind,
                "id": e.id,
                "tier": e.tier.map(EncounterTier::token),
                "reason": e.coverage.reason().unwrap_or_default(),
            }))
            .collect::<Vec<_>>(),
        // playtest-methodology.md rule 1: the binding count, stated out loud —
        // additive, never a substitute for `covered`/`not_covered`. `unbound`
        // is `examined == 0`; a `reason` accompanies it exactly then, because a
        // reader must never have to notice an empty pair of arrays to learn
        // this ledger matched nothing.
        "examined": floor_examined,
        "unbound": floor_examined == 0,
    });
    if floor_examined == 0 {
        floor_gate["reason"] = json!(FLOOR_GATE_UNBOUND_REASON);
    }

    let actors_examined = actors.len();
    let mut actors_gate = json!({
        "examined": actors_examined,
        "unbound": actors_examined == 0,
    });
    if actors_examined == 0 {
        actors_gate["reason"] = json!(ACTORS_GATE_UNBOUND_REASON);
    }

    json!({
        "version": plan.campaign.world.dsl_version,
        "campaign_id": plan.namespace,
        "difficulty": difficulty.token(),
        // The binding count for the whole spec-0023 pass: how many fights the
        // winnability proofs had to reason about, of BOTH shapes. `encounters`
        // below is the wave half only, and reading it as "how much combat is in
        // this delve" is what let a five-hostile campaign look combat-free.
        "fights": mandatory_fights(plan).to_json(),
        "encounters": entries,
        "actors": actors.iter().map(actor_json).collect::<Vec<_>>(),
        // Sibling of `actors[]`, not a rename of anything: how many actors this
        // build's tier machinery tracked at all. See the `combat_plan_json` doc
        // comment for why this and `floor_gate.unbound` are different questions.
        "actors_gate": actors_gate,
        "floor_gate": floor_gate,
    })
}

/// `floor_gate`'s reason when `examined == 0`: the ledger holds every wave and
/// actor billed `elite`/`boss` plus every untiered hostile actor,
/// so an empty ledger means none of those three things exist in the campaign —
/// a legitimate, common state (an all-`ordinary` delve) stated here so it is
/// never mistaken for a ledger that ran and found nothing.
const FLOOR_GATE_UNBOUND_REASON: &str = "no wave or actor in this campaign is billed \
    `elite`/`boss`, and no hostile actor goes untiered — the floor gate's ledger has \
    nothing to hold. This can be a legitimate build (e.g. an all-`ordinary` delve, or \
    one whose combat never crosses this gate's weight); it is stated explicitly so an \
    empty `covered`/`not_covered` pair is never read as a ledger that ran and passed.";

/// `actors_gate`'s reason when `examined == 0`: `actors[]` holds every actor
/// that declares ANY tier (`ordinary` included), so an empty array means no
/// actor in the campaign declares one at all — which is not the same fact as
/// "no hostile actor exists": an unleashed actor that never got a `tier` is
/// invisible here BY DESIGN (it lives in `floor_gate.not_covered` instead),
/// so this reason points a reader there rather than letting the
/// empty array read as "no actor combat".
const ACTORS_GATE_UNBOUND_REASON: &str = "no actor in this campaign declares a `tier` \
    (not even `ordinary`), so this build's actor-tier machinery tracked none. This is \
    NOT the same fact as \"no hostile actor exists\": an unleashed actor that declares \
    no tier at all does not appear here by design — check `floor_gate.not_covered` for \
    any UNTIERED hostile actor this may be masking.";

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
            drops: Vec::new(),
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
            drops: Vec::new(),
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
                drops: Vec::new(),
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
