//! Stage 5 — quests: each quest's trigger, objectives and effects, and the
//! stage document that holds every stage-5 collection.

pub mod check;
mod effect;
mod objective;
mod verb;

pub use effect::*;
pub use objective::*;
pub use verb::*;

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    Actor, Ambush, Assembly, CastEntry, EnvTrigger, Happening, LethalVolume, Loop, Loot, NpcId,
    ObjectiveId, Pulse, QuestId, Shop, Shortcut, Stake, StateDecl, TimedGate, Trap, Wave,
};

/// Stage 5 payload: quest expansions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuestsContent {
    /// The expanded quests (1:1 with stage 4).
    pub quests: Vec<Quest>,
    /// **How the campaign guides** (spec-0093, spec-0104): the defaults for
    /// every objective's `announcement` and `compass`, and the journal's manual.
    /// Absent = every default, byte for byte.
    #[serde(default, skip_serializing_if = "Guidance::is_default")]
    pub guidance: Guidance,
    /// Combat waves (DSL v0.3). Each wave is spawned by a `spawn-wave` effect and
    /// slain to complete a `kill` objective. Empty/absent = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waves: Vec<Wave>,
    /// Environment triggers (DSL v0.4, spec-0008 §7): "the world answers". Each
    /// watches an anchor for a strike / use / approach event and fires a bundle
    /// of [`QuestEffect`]s. Empty/absent = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<EnvTrigger>,
    /// Scripted actors (DSL v0.6, spec-0014): NoAI/Silent/no-loot puppets moved by
    /// compiler-emitted per-tick teleport. Distinct from stage-2 NPCs
    /// (no dialogue, any mob type). Summoned/removed/moved/unleashed by the actor
    /// staging effects. Empty/absent = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actors: Vec<Actor>,
    /// Traps (DSL v0.6, spec-0011; command payloads spec-0022): environmental
    /// hazards, each at one point anchor an area's prefab provides. Each says
    /// what springs the trap, what it then does (a command `payload`, a legacy
    /// dispenser `effect`, or both), how dangerous it is, how it is disarmed and
    /// whether it re-arms. Empty/absent = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traps: Vec<Trap>,
    /// Shortcut doors (spec-0016 §2): a gate that is sealed from world-load and
    /// is opened — permanently — from the FAR side. Empty/absent = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shortcuts: Vec<Shortcut>,
    /// Ambushes (spec-0016 §3): sugar over "deferred actors + a trigger that
    /// springs them". Empty/absent = none.
    ///
    /// **Never serialized.** [`parse_campaign`](crate::parse_campaign) expands
    /// each ambush into `triggers`, so the canonical form of a campaign is its
    /// **desugared** form. That is what keeps the canonical round-trip idempotent
    /// (re-parsing canonical output finds no `ambushes` and so cannot expand a
    /// second time and duplicate the trigger ids), and it means the sugar exists
    /// at exactly one layer boundary — the authored `.json` — with nothing
    /// downstream needing to know it was ever there. The list itself is kept in
    /// memory so diagnostics can name the ambush the author wrote.
    /// Timed gates (spec-0016 §4): gates on a deterministic open/close clock.
    /// Empty/absent = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timed_gates: Vec<TimedGate>,
    /// Container fills (spec-0021): pre-placed chests/barrels in the prefabs
    /// given contents at world init. Empty/absent = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loot: Vec<Loot>,
    /// **The container class** (spec-0105): what this campaign says about the
    /// containers its world holds that no fill and no device claims. Absent =
    /// none is scenery, which the build's container census proves (`DW1018`);
    /// present and empty is `DW1019`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containers: Option<crate::Containers>,
    /// Sign texts (spec-0128 §8): the words a sign the piece placed shows,
    /// written at world init. Empty/absent = none, byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signs: Vec<crate::Sign>,
    /// Runtime state data (DSL v0.10, spec-0031): named, scoped, integer-valued
    /// counters the campaign sets, adds to and clears at runtime, and compares
    /// against in any gate. A campaign that declares none emits none of it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state: Vec<StateDecl>,
    /// **The campaign's death beat** (DSL v0.10, spec-0031): effects run at the
    /// moment a player dies, for that player. Effect root **R7**
    /// ([`crate::EffectRootKind::OnDeath`]); a campaign that declares none emits
    /// none of it.
    ///
    /// **Why this is campaign-wide and not a field on a checkpoint.** The engine
    /// already has `on_respawn`, and it hangs off a `set-checkpoint` because
    /// *where you come back* is a property of the checkpoint. *That you died* is
    /// not: it is true at every point of the delve, under every checkpoint, and a
    /// bundle repeated on each checkpoint would be the same content written N
    /// times with N chances to forget one. So death is a moment in the campaign,
    /// and this is the one place it is named. Anything that should only happen in
    /// some phase of the delve is expressed by the ordinary per-effect
    /// `requires_flags` / `forbids_flags` gate every other root already carries —
    /// no second gating surface.
    ///
    /// **Audience is the dying player** (`Audience::Solo`, the audience
    /// `on_respawn` and `on_caught` already use): a death is one player's, and
    /// re-broadcasting it to the party would duplicate their narration and their
    /// kit. A beat the whole party should see is a `narrate` addressed by the
    /// author to the party through the effect's own vocabulary, not a different
    /// default here.
    ///
    /// **Timing.** It fires on the death edge while the player is still a corpse
    /// (`Health: 0.0f`, on the death screen) — see
    /// `emit::emit_checkpoint_functions`. That is the difference between this and
    /// `on_respawn`, which deliberately waits for the player to come back.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_death: Vec<QuestEffect>,
    /// Lethal volumes (DSL v0.10, spec-0031): declared boxes that kill whatever
    /// enters them. Empty/absent = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lethal_volumes: Vec<LethalVolume>,
    /// Shops (DSL v0.10, spec-0032): interaction points that open a list of
    /// gated offers. Empty/absent = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shops: Vec<Shop>,
    /// Recovery stakes (DSL v0.10, spec-0032): what a death forfeits, where the
    /// marker lands, and how it comes back. Empty/absent = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stakes: Vec<Stake>,
    /// Assemblies (spec-0082): fixed things built of display entities that
    /// play clips from a library rig, can be struck in melee, and strike back
    /// at a player who stands where they reach. Appear on `spawn-assembly`,
    /// leave on `despawn-assembly`. Empty/absent = nothing emitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assemblies: Vec<Assembly>,
    /// Loops (spec-0086): slabs whose crossing returns a body by a whole-block
    /// offset to an identical earlier section, held while a party gate is open.
    /// Empty/absent for every campaign that declares none, so such a campaign
    /// stays byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loops: Vec<Loop>,
    /// Pulses (spec-0102): sounds that beat from a mark on a fixed interval to
    /// every player standing in a place, while a party gate holds.
    /// Empty/absent for every campaign that declares none, so such a campaign
    /// stays byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pulses: Vec<Pulse>,
    #[serde(default, skip_serializing)]
    pub ambushes: Vec<Ambush>,
    /// Whether [`Self::expand_ambushes`] has already run (never serialized). The
    /// authored `ambushes` are deliberately KEPT after expansion so validation
    /// and the counterplay proof can attribute diagnostics to the ambush the
    /// author actually wrote; this flag is what makes a second expansion a no-op.
    #[serde(skip)]
    pub ambushes_expanded: bool,
}

/// Whether a piece of guidance is put in front of the player (spec-0093).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Shown — the announcement is printed, the compass points.
    #[default]
    Shown,
    /// Hidden — nothing is summoned or printed; the objective still adjudicates.
    Hidden,
}

impl Visibility {
    /// `true` for [`Visibility::Shown`]. Takes a reference so it doubles as the
    /// serde skip predicate on [`Guidance`]'s two fields.
    pub fn is_shown(&self) -> bool {
        *self == Visibility::Shown
    }
}

/// The campaign's guidance defaults (spec-0093, spec-0104): what an objective
/// gets when it states no `announcement` of its own.
///
/// Every value defaults to `shown`, so a document that omits the block is the
/// document every campaign already was. The objective's own field wins over
/// the campaign's; there is no third level, because a quest is where a beat is
/// booked and not the object guidance acts on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Guidance {
    /// The default for every objective's announcement — the `New objective`
    /// line, the hint's line, the cue sound and the `Objective complete` line.
    #[serde(default, skip_serializing_if = "Visibility::is_shown")]
    pub announcements: Visibility,
    /// The default for every objective's `compass` (spec-0104 §6): whether an
    /// announced objective is one the compass each player carries can follow.
    /// `hidden` with no objective opting in gives no compass at all.
    #[serde(default, skip_serializing_if = "Visibility::is_shown")]
    pub compass: Visibility,
    /// **The author's pages of the journal** (spec-0104 §8): written only for a
    /// setting that contradicts vanilla behaviour or an item that needs
    /// instructions. Absent is the default and the right choice for most
    /// delves; the journal's compass page and its open and done objectives are
    /// the engine's, derived, never authored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<Manual>,
}

/// The author's part of the journal (spec-0104 §8.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manual {
    /// The journal's name in the hand. Absent = the engine's `Journal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// One string per book page, each a line of the span grammar (spec-0096)
    /// that names any key through the key span (`[[key|use]]`). At least one.
    #[serde(deserialize_with = "non_empty_pages")]
    #[schemars(length(min = 1))]
    pub pages: Vec<String>,
}

/// `pages` may not be empty when `manual` is declared: an empty manual is a
/// declaration of nothing, refused where it is entered.
fn non_empty_pages<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let pages = Vec::<String>::deserialize(d)?;
    if pages.is_empty() {
        return Err(serde::de::Error::custom(
            "`guidance.manual.pages` is empty: a manual holds at least one page; remove \
             `manual` to write none",
        ));
    }
    Ok(pages)
}

/// **A highlight the party is near** (spec-0104 §7): the act's block or body
/// glows while a player stands within `within` blocks of it and the act is
/// armed. Declared by the author per act; never inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Highlight {
    /// The range in blocks, 1 to 64 (64 is the farthest an outline is drawn).
    #[serde(deserialize_with = "highlight_within")]
    #[schemars(range(min = 1, max = 64))]
    pub within: u8,
}

/// `within` is 1..=64, refused where it is entered.
fn highlight_within<'de, D>(d: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let n = u64::deserialize(d)?;
    if !(1..=64).contains(&n) {
        return Err(serde::de::Error::custom(format!(
            "`highlight.within` is {n}: the range is 1 to 64 blocks (an outline is drawn to \
             64 blocks at most)"
        )));
    }
    Ok(n as u8)
}

impl Guidance {
    /// Serde skip predicate: every field at its default needs no block on the
    /// wire.
    pub fn is_default(&self) -> bool {
        *self == Guidance::default()
    }
}

impl QuestsContent {
    /// Every environment trigger this stage produces: the authored `triggers`
    /// followed by the ones each `ambush` desugars to (spec-0016 §3), in
    /// declared order.
    ///
    /// **This is the single trigger authority.** Validation, the l10n
    /// inventory, the flag/wave producer scans, the nav proofs and emission all
    /// read triggers through it, so an ambush behaves exactly like the trigger
    /// an author would otherwise hand-write — there is no second code path for
    /// the sugar to drift down. Deterministic (declaration order, no hashing).
    pub fn all_triggers(&self) -> Vec<EnvTrigger> {
        let mut out = self.triggers.clone();
        out.extend(self.ambushes.iter().map(Ambush::to_trigger));
        out
    }

    /// **The triggers that answer a press on the gate `gate`** (spec-0117 §6):
    /// every `use` trigger that declares `serves: {gate}` naming it, in
    /// declared order — the gate's own answer at a cell of its region, or a
    /// knocker beside it.
    ///
    /// One predicate for every reader of "this sealed gate answers a press":
    /// `DW0429`'s obligation on a sealed gate (`compiler::pressable::check`)
    /// and `DW1054`'s confirmation of a way declared `told` (spec-0118 §6.2,
    /// `compiler::nav::learned`). A `told` way's press answer is a press
    /// answer, so the two read one rule.
    pub fn gate_answers<'a>(&'a self, gate: &'a str) -> impl Iterator<Item = &'a EnvTrigger> + 'a {
        self.triggers.iter().filter(move |t| {
            matches!(t.on, crate::TriggerOn::Use)
                && matches!(&t.serves, Some(crate::Serves::Gate(g)) if g.as_str() == gate)
        })
    }

    /// Desugar every `ambush` into a real environment trigger and clear the
    /// ambush list (spec-0016 §3). Called once, by
    /// [`parse_campaign`](crate::parse_campaign); idempotent by construction
    /// (a second call sees no ambushes left to expand).
    pub fn expand_ambushes(&mut self) {
        if self.ambushes_expanded || self.ambushes.is_empty() {
            return;
        }
        self.triggers = self.all_triggers();
        self.ambushes_expanded = true;
    }

    /// The declared datum with this id, if any (DSL v0.10).
    pub fn state_decl(&self, id: &str) -> Option<&StateDecl> {
        self.state.iter().find(|s| s.id.as_str() == id)
    }

    /// The declared stake with this id, if any (DSL v0.10, spec-0032).
    pub fn stake_decl(&self, id: &str) -> Option<&Stake> {
        self.stakes.iter().find(|s| s.id.as_str() == id)
    }

    /// The declared assembly with this id, if any (spec-0082).
    pub fn assembly_decl(&self, id: &str) -> Option<&Assembly> {
        self.assemblies.iter().find(|a| a.id.as_str() == id)
    }
}

/// One expanded quest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Quest {
    /// Quest id (matches a stage-4 planned quest).
    pub id: QuestId,
    /// What starts the quest.
    pub trigger: Trigger,
    /// Ordered objectives (intra-quest DAG via `after`).
    pub objectives: Vec<Objective>,
    /// Effects fired when a given objective completes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub on_objective_complete: BTreeMap<ObjectiveId, Vec<QuestEffect>>,
    /// Effects fired when the whole quest completes.
    pub on_complete: Vec<QuestEffect>,
    /// The **cast ledger** (DSL v0.7, spec-0020): for every stage-2 NPC that is
    /// live during this quest — spawned and not explicitly removed — where it
    /// stands, what it is doing, and what its right-click offers *for this
    /// quest's duration*.
    ///
    /// The ledger exists because an NPC's dialogue used to be one tree for the
    /// whole campaign: after the climactic escape a crew member still offered
    /// "Tell me what he is." — a premise question absurd once the story moved on.
    /// Declaring the scene per quest makes the compiler able to check it
    /// (`DW0460`–`DW0467`) and makes the declaration itself the gate: the
    /// emitted right-click shows the root this quest declares, so a stale root
    /// retires *because the ledger says so*, not because an author remembered a
    /// flag.
    ///
    /// Every NPC live during the quest owes an entry (`DW0460`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cast: BTreeMap<NpcId, CastEntry>,
    /// What this quest does to the story (DSL v0.8, spec-0025; required at 0.8.0,
    /// `DW0481`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub happening: Option<Happening>,
}

impl Quest {
    /// The acts `o`'s completion performs (spec-0114 §4), read with the
    /// effects this quest fires when it completes
    /// ([`Objective::completion_acts`]).
    pub fn objective_acts(&self, o: &Objective) -> std::collections::BTreeSet<Act> {
        o.completion_acts(self.objective_completion_effects(o))
    }

    /// The kind and fields `o`'s acts are derived from
    /// ([`Objective::completion_shape`]).
    pub fn objective_shape(&self, o: &Objective) -> String {
        o.completion_shape(self.objective_completion_effects(o))
    }

    /// The effects this quest fires when `o` completes.
    pub fn objective_completion_effects(&self, o: &Objective) -> &[QuestEffect] {
        self.on_objective_complete
            .get(o.id())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// What triggers a quest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Trigger {
    /// Fires when the campaign starts.
    CampaignStart,
    /// Fires when another quest completes.
    QuestComplete {
        /// The prerequisite quest.
        quest: QuestId,
    },
}
