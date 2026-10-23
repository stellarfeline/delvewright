//! Environment triggers and the props a player interacts with.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::{default_true, is_false};
use crate::{AnchorId, AssemblyId, FlagId, NpcId, QuestEffect, StateCompare, TriggerId};

#[cfg(doc)]
use crate::stepped_blocks;

/// A stage-5 environment trigger (DSL v0.4). Emission uses vanilla-intended
/// primitives only (spec-0008 §7): `strike`/`use` read a `minecraft:interaction`
/// entity's attack/interaction records; `approach` is a `distance` selector on
/// the tick. Look-at / break-attempt detection is excluded on principle (no
/// vanilla primitive).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvTrigger {
    /// Unique trigger id (`trigger/<kebab>`).
    pub id: TriggerId,
    /// The anchor this trigger watches. Required for `strike` / `use` /
    /// `approach`, which watch a *place*; **absent** for `strike-npc` (DSL
    /// v0.6), which watches a *character* and names it in `on.npc` instead —
    /// there is no cell for the author to supply and no cell the compiler
    /// would use. Either mismatch is `DW0194`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<AnchorId>,
    /// **The real block the click acts on** (spec-0117 §3a): for a `use` or a
    /// `strike` trigger at a point anchor, the block the compiler places at
    /// `at` from `setup_finish` to the end of the delve — any block with an
    /// outline. A bell or a button is pressed on its own (vanilla reports the
    /// press and it changes nothing the world keeps); a lever is pressed on its
    /// own where every press acts; every other block, and every `strike`, is
    /// pressed through an invisible click box fitted to the block's outline.
    /// Required there (`DW1059`); a `prop` on an event with no cell of its own —
    /// an approach, a step, a `strike-npc`, a `strike-assembly` — is `DW0964`;
    /// on a gate's own answer (a `serves: {gate}` trigger anchored inside the
    /// gate region) it is `DW1052`, because the gate names its block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prop: Option<Prop>,
    /// **What the press answers outside its window** (spec-0117 §4): one
    /// `Refusal` line per state the trigger has — `not_yet` while its gate is
    /// shut, `done` once it is settled (`once` fired, or its `serves` referent
    /// settled). Each line is owed where its state exists (`DW1051`) and
    /// refused where it does not (`DW1052`); the engine never writes the
    /// words. Admitted on every kind that names its actor; refused on an
    /// `approach` (`DW0427`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answers: Option<Answers>,
    /// **The block is vanilla's once the press settles** (spec-0117 §3a): a
    /// boxed block that acts on its own (a door, a lever with a settled state,
    /// a chest) is not boxed again after settlement and answers nothing — its
    /// own act is the answer. Default `false`. Refused on a block pressed on its
    /// own and on a trigger with no settled state (`DW1052`), and on a block the
    /// game reports no press of (`DW1051`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub release: bool,
    /// **The thing this press is part of** (spec-0117 §4): an objective, or a
    /// gate a `close-gate` or a `shortcut` seals. The trigger is open only while
    /// its own gate holds **and** the objective is pending (or the gate is
    /// sealed), and settled once the objective is (or the gate opens) — so a
    /// puzzle stops accepting presses the moment it is solved, whatever `once`
    /// says. A `serves: {gate}` trigger anchored inside the gate region is the
    /// gate's own answer: its body is the gate's block there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serves: Option<Serves>,
    /// The proximity highlight (spec-0104 §7): the block this trigger is
    /// pressed on, or the body it strikes, glows while a player is within
    /// `within` blocks and the trigger is armed. An `approach`, a `step` and a
    /// `strike-assembly` have nothing to light (`DW1016`). Absent = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<crate::Highlight>,
    /// The event that fires it.
    pub on: TriggerOn,
    /// Flags that must be set before the trigger can fire (DSL v0.4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Negative flag gate (DSL v0.6): the trigger is
    /// **suppressed** while ANY listed flag is set (by any player — flags are
    /// campaign state). The dual of `requires_flags`, so an "armed between two
    /// story beats" trigger needs no re-arm plumbing: e.g. a strike-the-giant
    /// retaliation trigger with `requires_flags: [flag/sealed]` and
    /// `forbids_flags: [flag/asleep]` arms when the cave seals and stands down
    /// the moment the wake beat takes over.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Numeric gate terms (DSL v0.10, spec-0031): every listed comparison must
    /// hold for this gate to be open. The third field of the one gate, carried by
    /// every gate consumer — never by the verb that first wanted it. Default
    /// empty: no numeric term.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_state: Vec<StateCompare>,
    /// Fire at most once (default `true`, mirroring objective completion). Set
    /// `false` to allow re-firing every time the condition is met.
    #[serde(default = "default_true")]
    pub once: bool,
    /// **Who the trigger's effects address** (DSL v0.11). Default
    /// [`TriggerAudience::Party`], so every campaign written before this field
    /// existed is byte-identical.
    ///
    /// A trigger is two different things depending on what the author means by
    /// it. A pressure plate that opens a gate and narrates the room is a **party
    /// beat**: everyone should see it, and it does not matter who stepped on the
    /// plate. A barred door that answers *"this cannot be opened from this
    /// side"* is a **reply to one person**: broadcasting it tells four players
    /// about a door three of them are nowhere near.
    ///
    /// Until this field the second was inexpressible, which is why the two verbs
    /// that needed it (`close-gate`'s seal answer, and nothing at all for a
    /// shortcut door) grew their own private reply machinery instead. The
    /// capability belongs to the press, not to the verb.
    #[serde(default, skip_serializing_if = "TriggerAudience::is_party")]
    pub audience: TriggerAudience,
    /// Effects fired when the trigger matches.
    pub effects: Vec<QuestEffect>,
}

impl EnvTrigger {
    /// The anchor this trigger watches, if it watches a place at all. `None`
    /// for `strike-npc`, whose target is a character.
    pub fn at_anchor(&self) -> Option<&str> {
        self.at.as_ref().map(|a| a.as_str())
    }

    /// Whether this trigger's bundle is addressed to the player who pressed it.
    pub fn addresses_presser(&self) -> bool {
        self.audience == TriggerAudience::Presser
    }

    /// Whether vanilla can name the player whose act fired this trigger, which
    /// is what `audience: presser` needs: a right-click (`use`, through
    /// `minecraft:player_interacted_with_entity`), a left-click on an
    /// interaction entity (`strike`, `strike-npc`, `strike-assembly`, through
    /// `minecraft:player_hurt_entity`, measured in
    /// `docs/reference/click-attribution.md`) and a step (`step`, a player in
    /// the cell). An `approach` has no carrier entity, so nothing names its
    /// player: it alone is refused by `DW0427`. The DSL-side statement of the
    /// compiler's criterion table (`emit::trigger::press_criterion`), held
    /// equal to it by an agreement test.
    pub fn attributes_its_actor(&self) -> bool {
        self.on.attributes_its_actor()
    }
}

/// Who an [`EnvTrigger`]'s effects address (DSL v0.11).
///
/// **This is a dispatch decision, not a cosmetic one.** A `party` trigger is
/// polled on the tick with no executor, so `@s` does not exist and every
/// player-facing command addresses `@a`. A `presser` trigger is dispatched by an
/// advancement whose reward runs a function *as the player who clicked*, so `@s`
/// is the presser and the bundle addresses them alone: a right-click (`use`) by
/// `minecraft:player_interacted_with_entity`, a left-click on an interaction
/// entity (`strike`, `strike-npc`, `strike-assembly`) by
/// `minecraft:player_hurt_entity`, which vanilla grants to the attacking player
/// when the struck entity is a `minecraft:interaction`
/// (`docs/reference/click-attribution.md`, measured on the pinned jar). A
/// `presser` trigger `on: step` is polled as `execute as @a[<the cell>]`, so
/// `@s` is each player who stepped on, on their own step.
///
/// An `approach` has no carrier entity and no criterion names its player, so
/// `presser` on one is refused (`DW0427`) rather than approximated: per
/// CLAUDE.md's no-hack rule, a capability with no vanilla primitive under it is
/// excluded, never faked downstream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TriggerAudience {
    /// The whole party (the default).
    #[default]
    Party,
    /// The one player whose press fired it: the right-click of a `use`, the
    /// left-click of a `strike`, `strike-npc` or `strike-assembly`, the step of
    /// a `step`.
    Presser,
}

impl TriggerAudience {
    /// Serde skip predicate: the default needs no field on the wire, so the
    /// canonical form of a trigger that states the default carries none.
    fn is_party(&self) -> bool {
        *self == TriggerAudience::Party
    }
}

/// The event an [`EnvTrigger`] watches (DSL v0.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "on", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TriggerOn {
    /// The player attacks (left-clicks) the interaction entity at the anchor.
    Strike,
    /// The player uses (right-clicks) the interaction entity at the anchor.
    Use,
    /// The player comes within `range` blocks of the anchor.
    Approach {
        /// Approach radius (blocks).
        range: u32,
    },
    /// A player steps onto the anchor's cell, which holds a block a step fires
    /// — a pressure plate or the tripwire string ([`stepped_blocks`]); the
    /// piece places it, and a cell that holds anything else is `DW0917`.
    ///
    /// Detected as a player whose hitbox is in the cell (the selector a plate
    /// or tripwire trap fires on), edge-latched so standing on the plate fires
    /// once. With `audience: presser` each player who steps on is dispatched
    /// as `@s` on their own step: the act and the actor are the same fact, a
    /// body in the cell, so no player is inferred after the event.
    Step,
    /// The player attacks (left-clicks) an **NPC's body** (DSL v0.6).
    ///
    /// The place-based [`TriggerOn::Strike`] cannot express "hit the giant": it
    /// summons its own `minecraft:interaction` at a *cell*, and a large NPC's
    /// body eclipses that cell (`DW0359`), so the click never reaches the
    /// trigger — the owner's island round-7 finding. This form has no cell. It
    /// rides the interaction entity the NPC already owns, which is the entity a
    /// click on that NPC reaches by construction.
    ///
    /// Right-click and left-click stay separate all the way down: a
    /// `minecraft:interaction` records them in two distinct NBT fields
    /// (`interaction` and `attack`), so the NPC's dialogue keeps the right-click
    /// and this trigger takes the left-click, on one shared hitbox.
    StrikeNpc {
        /// The NPC (stage-2 ref) whose body is the target.
        npc: NpcId,
    },
    /// The player attacks (left-clicks) an **assembly's hitbox** (spec-0082).
    ///
    /// The exact shape of [`TriggerOn::StrikeNpc`]: no `at`, because the target
    /// is an object with a hitbox of its own, and the trigger rides it. Melee
    /// only — the hitbox is a `minecraft:interaction`, and an arrow passes
    /// through one without writing its `attack` record (spec-0082 §8 row 5).
    /// `once: false` with an `add-state` is how a hit count is built.
    StrikeAssembly {
        /// The assembly (stage-5 `assemblies` ref) whose hitbox is the target.
        assembly: AssemblyId,
    },
}

impl TriggerOn {
    /// The kebab tag (`strike` / `use` / `approach` / `step` / `strike-npc` /
    /// `strike-assembly`).
    pub fn kind(&self) -> &'static str {
        match self {
            TriggerOn::Strike => "strike",
            TriggerOn::Use => "use",
            TriggerOn::Approach { .. } => "approach",
            TriggerOn::Step => "step",
            TriggerOn::StrikeNpc { .. } => "strike-npc",
            TriggerOn::StrikeAssembly { .. } => "strike-assembly",
        }
    }

    /// Whether this event is a click on a `minecraft:interaction` hitbox — a
    /// `strike`, a `use`, a `strike-npc`, a `strike-assembly`. An `approach`
    /// and a `step` are a body's position, read on the tick, and have no
    /// hitbox.
    pub fn is_click(&self) -> bool {
        matches!(
            self,
            TriggerOn::Strike
                | TriggerOn::Use
                | TriggerOn::StrikeNpc { .. }
                | TriggerOn::StrikeAssembly { .. }
        )
    }

    /// Whether this event needs an `at` anchor — true for everything that
    /// watches a place, false for `strike-npc` and `strike-assembly`, which
    /// watch an object that carries its own hitbox.
    pub fn needs_anchor(&self) -> bool {
        !matches!(
            self,
            TriggerOn::StrikeNpc { .. } | TriggerOn::StrikeAssembly { .. }
        )
    }

    /// Whether vanilla names the player whose act this event is — every event
    /// but an `approach`, which has no carrier entity. The one table
    /// [`EnvTrigger::attributes_its_actor`] reads.
    pub fn attributes_its_actor(&self) -> bool {
        !matches!(self, TriggerOn::Approach { .. })
    }

    /// The assembly whose hitbox this event watches (`strike-assembly` only).
    pub fn assembly_target(&self) -> Option<&AssemblyId> {
        match self {
            TriggerOn::StrikeAssembly { assembly } => Some(assembly),
            _ => None,
        }
    }

    /// The NPC whose body this event watches (`strike-npc` only).
    pub fn npc_target(&self) -> Option<&NpcId> {
        match self {
            TriggerOn::StrikeNpc { npc } => Some(npc),
            _ => None,
        }
    }
}

/// The real block a pressable is (spec-0117 §3a): placed by the compiler at the
/// pressable's cell from `setup_finish` to the end of the delve, drawn and
/// occluded like the world. Its id is validated against the pinned 1.21.11 block
/// registry (`DW0193`), and it must have an outline (`DW1059`).
///
/// **The block decides how it is pressed.** A bell or a button — and a lever
/// on a pressable whose every press acts — is pressed **on its own**: vanilla
/// reports the press (`default_block_use` at the cell) and the press changes
/// nothing the world keeps. Every other block is pressed **through a click
/// box** — an invisible `minecraft:interaction` fitted to the block's own
/// outline — so a door does not open, a lever does not flip and a chest does
/// not open its screen while the press is not its turn. A door is named by its
/// lower half; the compiler places both.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Prop {
    /// Vanilla block id, with an optional blockstate suffix (e.g.
    /// `minecraft:lever[face=floor,facing=north]`).
    pub block: String,
}

impl Prop {
    /// Whether this prop's block is one a hand presses whose use vanilla
    /// reports — a lever, a button, a bell (spec-0117 §2a).
    pub fn is_hand_pressed(&self) -> bool {
        crate::blockshape::is_hand_pressed(&self.block)
    }
}

/// **What a pressable answers when the press is not its turn** (spec-0117 §4):
/// one `Refusal` line per state, put on the presser's action bar with the
/// engine's refusal cue. Each line is optional in the schema; which are owed is
/// `DW1051`'s, which are inert `DW1052`'s, and the engine never writes the
/// words.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Answers {
    /// Said to a press **before** its window — the step has not come yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_yet: Option<String>,
    /// Said to a press **inside** the window of an `interact` with
    /// `requires_item` when the hand does not hold the item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_item: Option<String>,
    /// Said to a press **after** the act is done — the settled state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<String>,
}

impl Answers {
    /// The line declared for `state`, if any.
    pub fn line(&self, state: AnswerState) -> Option<&String> {
        match state {
            AnswerState::NotYet => self.not_yet.as_ref(),
            AnswerState::MissingItem => self.missing_item.as_ref(),
            AnswerState::Done => self.done.as_ref(),
        }
    }

    /// Every declared line with its state, in [`AnswerState::ALL`] order.
    pub fn declared(&self) -> Vec<(AnswerState, &String)> {
        AnswerState::ALL
            .iter()
            .filter_map(|s| self.line(*s).map(|l| (*s, l)))
            .collect()
    }
}

/// One of the three states a press can be answered in (spec-0117 §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnswerState {
    /// Shut: the window has not opened.
    NotYet,
    /// Open, with the hand empty of the required item.
    MissingItem,
    /// Settled: the act is done.
    Done,
}

impl AnswerState {
    /// Every state, in the order the lines are written.
    pub const ALL: [AnswerState; 3] = [
        AnswerState::NotYet,
        AnswerState::MissingItem,
        AnswerState::Done,
    ];

    /// The field name (`not_yet`, `missing_item`, `done`): the l10n key's last
    /// segment.
    pub fn field(self) -> &'static str {
        match self {
            AnswerState::NotYet => "not_yet",
            AnswerState::MissingItem => "missing_item",
            AnswerState::Done => "done",
        }
    }

    /// The kebab token a synthesized answer trigger's id ends with.
    pub fn token(self) -> &'static str {
        match self {
            AnswerState::NotYet => "not-yet",
            AnswerState::MissingItem => "missing-item",
            AnswerState::Done => "done",
        }
    }

    /// The `#ans_<owner>` trace value an answer of this state writes (1, 2, 3).
    pub fn trace(self) -> u8 {
        match self {
            AnswerState::NotYet => 1,
            AnswerState::MissingItem => 2,
            AnswerState::Done => 3,
        }
    }
}

/// **The thing a trigger's press is part of** (spec-0117 §4): one field whose
/// referent's kind the object states.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Serves {
    /// An objective: the press is open only while it is pending, settled once
    /// it is complete.
    Objective(crate::ObjectiveId),
    /// A gate a `close-gate` or a `shortcut` seals: the press is open only while
    /// it is sealed, settled once it is open.
    Gate(AnchorId),
}

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::quest::check::check_effect_v04;
use crate::registry::{AnchorRegistry, BlockRegistry};
use crate::validate::{
    AnchorProviders, check_block_field, for_each_trigger_effect_deep, station_kind_diag,
};
use std::collections::{BTreeMap, BTreeSet};

crate::dw_code! {
    /// (v0.4) An environment trigger id is malformed (`DW0110`-style) or
    /// duplicated within the stage-5 `triggers` namespace.
    pub const TRIGGER_INVALID: DwCode = DwCode::new("DW0194", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.4, added round-6) A `use` trigger anchored where an NPC stands.
    /// Right-click on an NPC already belongs to its dialogue advancement; a
    /// second interaction hitbox in the same cell makes the client's entity
    /// ray-pick ambiguous, and whichever entity loses the tie is silently dead
    /// — the round-6 island soft-lock class (an exactly co-located hitbox
    /// starved the giant's dialogue of every right-click). `strike` triggers
    /// are exempt: a left-click has no dialogue meaning, so the compiler rides
    /// the trigger's tag on the NPC's own hitbox instead of summoning a second
    /// one. Validation-tier (exit 1).
    pub const USE_TRIGGER_ON_NPC: DwCode = DwCode::new("DW0350", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.11; spec-0119) **A press answer addressed to an event nobody is named
    /// for.** A trigger declares `audience: presser` on an `on: approach`.
    ///
    /// A presser is named by the event's own carrier: a right-click on an
    /// interaction entity by `minecraft:player_interacted_with_entity`, a
    /// left-click on one by `minecraft:player_hurt_entity`
    /// (`docs/reference/click-attribution.md`), a step by the positional
    /// selector over the cell. An `approach` is a distance test on the tick with
    /// no carrier entity, so no criterion names its player. Approximating it
    /// (assuming the nearest player) is the downstream folklore CLAUDE.md's
    /// no-hack rule excludes, so the capability is refused rather than faked.
    pub const TRIGGER_AUDIENCE_UNATTRIBUTABLE: DwCode = DwCode::new("DW0427", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.11) **A trigger id in the compiler's reserved `dw-` namespace.** The
    /// compiler synthesizes triggers of its own — today the press answer every
    /// pressable gives (`trigger/dw-answer-…`) — and two
    /// triggers sharing an id would share one `dw_trig_…` tag and one emitted
    /// function, so one of them would silently disappear. Reserving the prefix
    /// makes the collision impossible by construction instead of improbable.
    pub const TRIGGER_ID_RESERVED: DwCode = DwCode::new("DW0428", ExitTier::Build);
}

/// `DW0427`/`DW0428`: the two ways a trigger's **press answer** surface can be
/// declared wrong (DSL v0.11; spec-0117 adds `answers` to `DW0427`).
///
/// Both are about the trigger as an *object*, not about any effect inside it, so
/// they sit together and are checked over the one trigger authority.
pub(crate) fn press_answer_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, t) in c.quests.content.triggers.iter().enumerate() {
        if t.answers.is_some() && !t.attributes_its_actor() {
            d.push(Diagnostic::error(
                TRIGGER_AUDIENCE_UNATTRIBUTABLE,
                "quests",
                format!("/content/triggers/{i}/answers"),
                format!(
                    "trigger `{}` watches a `{}` and declares `answers`, but an answer is told to \
                     the one player who acted, and an approach names nobody: it is a distance \
                     test on the tick, with no entity a criterion could report. Prescription: \
                     make it a `use`, `strike` or `step` on something the player presses or \
                     steps on, or drop `answers`",
                    t.id,
                    t.on.kind()
                ),
            ));
        }
        if t.addresses_presser() && !t.attributes_its_actor() {
            d.push(Diagnostic::error(
                TRIGGER_AUDIENCE_UNATTRIBUTABLE,
                "quests",
                format!("/content/triggers/{i}/audience"),
                format!(
                    "trigger `{}` watches a `{}` and asks for `audience: presser`, but a presser \
                     needs a carrier that names the player, and an approach has none: it is a \
                     distance test on the tick, with no entity a criterion could report. A click \
                     names its player through its interaction entity (a right-click by \
                     `minecraft:player_interacted_with_entity`, a left-click by \
                     `minecraft:player_hurt_entity`), and a step through the cell's selector. \
                     Guessing — hoping the nearest player is the one who came — is the kind of \
                     downstream folklore this engine refuses (CLAUDE.md: a capability with no \
                     vanilla primitive under it is excluded, not faked). Prescription: make it a \
                     `use`, `strike` or `step` trigger on something the player presses, or drop \
                     `audience` and let the beat address the party",
                    t.id,
                    t.on.kind()
                ),
            ));
        }
        let local = crate::l10n::local_id(t.id.as_str());
        if local.starts_with(RESERVED_TRIGGER_PREFIX) {
            d.push(Diagnostic::error(
                TRIGGER_ID_RESERVED,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "trigger id `{}` opens with `{RESERVED_TRIGGER_PREFIX}`, which the compiler \
                     reserves for the triggers it synthesizes itself — every pressable's \
                     answer (`trigger/dw-answer-…`). Two \
                     triggers with one id would share one `dw_trig_…` tag and one emitted \
                     function, so one of them would silently vanish. Prescription: rename it; any \
                     kebab id not starting with `{RESERVED_TRIGGER_PREFIX}` is yours",
                    t.id
                ),
            ));
        }
    }
}

/// The id prefix the compiler reserves for triggers it synthesizes
/// (`plan::press_answer_trigger_id`). Stated here because the *reservation* is a
/// DSL-level fact even though today's only user is in the compiler.
const RESERVED_TRIGGER_PREFIX: &str = "dw-";

/// Every effect anchor of an environment trigger (`DW0142`/`DW0871`), at any
/// nesting depth, resolved against the union of every known area's anchors.
pub(crate) fn trigger_anchor_checks(
    c: &Campaign,
    providers: &AnchorProviders,
    d: &mut Vec<Diagnostic>,
) {
    // Environment triggers are global (no owning area), so their effect anchors
    // resolve against the union of every known area's anchors — the same
    // resolved-or-diagnostic rule as quest effects, applied at the only scope a
    // trigger has. Skipped entirely when some area binds a pool / an unknown
    // prefab, because then the union is not the whole truth.
    if providers.all_areas_known() {
        for (ti, t) in c.quests.content.triggers.iter().enumerate() {
            for_each_trigger_effect_deep(t, |path, eff| {
                for (suffix, anchor, demands) in eff.anchor_refs() {
                    if let Some(f) = station_kind_diag(
                        providers,
                        anchor.as_str(),
                        demands,
                        &format!("`{}`", eff.verb.tag()),
                        "quests",
                        format!("/content/triggers/{ti}/{path}/{suffix}"),
                    ) {
                        d.push(f);
                        continue;
                    }
                    if providers.union().contains(anchor.as_str()) {
                        continue;
                    }
                    d.push(Diagnostic::error(
                        codes::ANCHOR_UNRESOLVED,
                        "quests",
                        format!("/content/triggers/{ti}/{path}/{suffix}"),
                        format!(
                            "`{verb}` anchor `{anchor}` in an environment trigger is not \
                             provided by any area's prefab — {}",
                            providers.anchor_remedy(
                                "use an anchor a prefab exposes (anchor names come from prefab \
                                 metadata; do NOT invent one)"
                            ),
                            verb = eff.verb.tag(),
                        ),
                    ));
                }
            });
        }
    }
}

/// An environment trigger's effect `requires_flags` / `forbids_flags`, at any
/// nesting depth, name a produced flag (`DW0172`).
pub(crate) fn trigger_effect_flag_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;

    let declared_flags: BTreeSet<String> = crate::validate::produced_flags(c);

    // v0.6: environment-trigger effect `requires_flags` / `forbids_flags`
    // resolution (DW0172).
    for (i, t) in quests.triggers.iter().enumerate() {
        for_each_trigger_effect_deep(t, |path, eff| {
            for (n, f) in eff.requires_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/triggers/{i}/{path}/when/requires_flags/{n}"),
                        format!(
                            "effect `requires_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                             earlier, or correct the flag name"
                        ),
                    ));
                }
            }
            for (n, f) in eff.forbids_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/triggers/{i}/{path}/when/forbids_flags/{n}"),
                        format!(
                            "effect `forbids_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — the gate can never suppress anything; add the \
                             producing `set-flag {{ flag: \"{f}\" }}` effect, or correct the flag \
                             name"
                        ),
                    ));
                }
            }
        });
    }
}

/// A trigger's `prop` block id is in the block registry (`DW0193`).
pub(crate) fn trigger_prop_checks(
    c: &Campaign,
    blocks: &dyn BlockRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;

    // --- block ids: interact props + set-block effects (quest + trigger) ---
    // A trigger's `prop` (spec-0093 §6.5) is the same object class as an
    // interact's and is held to the same registry.
    for (i, t) in quests.triggers.iter().enumerate() {
        if let Some(prop) = &t.prop {
            check_block_field(
                blocks,
                &prop.block,
                format!("/content/triggers/{i}/prop/block"),
                "triggers[].prop",
                "minecraft:lever[face=floor,facing=north]",
                d,
            );
        }
    }
}

/// Environment trigger declarations (spec-0008 §7): id syntax and uniqueness
/// (`DW0194`), the `at` / `strike-npc` target (`DW0194`, `DW0142`, `DW0112`), an
/// `approach` range, a `use` trigger on an NPC's cell (`DW0350`), the trigger's
/// own flags (`DW0172`) and every effect's references ([`check_effect_v04`]).
pub(crate) fn trigger_decl_checks(
    c: &Campaign,
    anchors: &dyn AnchorRegistry,
    blocks: &dyn BlockRegistry,
    flags: &BTreeSet<&str>,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let declared_waves: BTreeSet<&str> = quests.waves.iter().map(|w| w.id.as_str()).collect();

    // area anchor sets (single-prefab areas only) + whether any pool area exists.
    let providers = AnchorProviders::build(c, anchors);

    // --- environment triggers ---
    let mut seen_triggers: BTreeSet<&str> = BTreeSet::new();
    for (i, t) in quests.triggers.iter().enumerate() {
        if !t.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "malformed trigger id `{}` — trigger ids must be lowercase kebab-case with \
                     the `trigger/` prefix (e.g. `trigger/pressure-plate`)",
                    t.id
                ),
            ));
        }
        if !seen_triggers.insert(t.id.as_str()) {
            d.push(Diagnostic::error(
                TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "duplicate trigger id `{}` — rename one so every trigger id is unique",
                    t.id
                ),
            ));
        }
        // `at` names a place; `strike-npc` names a character. Exactly one of the
        // two must be supplied, so neither form can be authored half-way (an
        // ignored anchor would read as meaningful and silently do nothing).
        match (t.on.needs_anchor(), t.at_anchor()) {
            (true, None) => d.push(Diagnostic::error(
                TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "trigger `{}` fires on `{}`, which watches a place, but declares no `at` \
                     anchor — add one ({}), or switch to `strike-npc` if the target is an NPC's \
                     body",
                    t.id,
                    t.on.kind(),
                    providers
                        .anchor_remedy("anchor names come from prefab metadata; do NOT invent one"),
                ),
            )),
            (false, Some(at)) => d.push(Diagnostic::error(
                TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "trigger `{}` fires on `{}`, whose target is {} — it watches no cell, so \
                     the `at` anchor `{at}` names nothing and would be silently ignored. \
                     Remove `at`.",
                    t.id,
                    t.on.kind(),
                    match (t.on.npc_target(), t.on.assembly_target()) {
                        (Some(n), _) => format!("NPC `{n}`'s body"),
                        (_, Some(m)) => format!("assembly `{m}`'s hitbox"),
                        _ => "an object".to_string(),
                    }
                ),
            )),
            (true, Some(at)) if !providers.resolvable(at) => d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "trigger `at` anchor `{at}` is not provided by any area's prefab — {}",
                    providers.anchor_remedy(
                        "set `at` to an anchor some area's prefab exposes (anchor names come from \
                         prefab metadata; do NOT invent one)"
                    ),
                ),
            )),
            _ => {}
        }
        // A `strike-npc` target must be a real stage-2 NPC: the trigger's tag
        // rides that NPC's hitbox, so an unknown id would emit a tag on nothing
        // and the trigger could never fire.
        if let Some(npc) = t.on.npc_target()
            && !c.npcs.content.npcs.iter().any(|n| n.id == *npc)
        {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                format!("/content/triggers/{i}/on/npc"),
                format!(
                    "`strike-npc` trigger `{}` targets NPC `{npc}`, which stage 2 does not \
                     declare — use a declared npc id",
                    t.id
                ),
            ));
        }
        if let TriggerOn::Approach { range } = &t.on
            && *range == 0
        {
            d.push(Diagnostic::error(
                TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/on/range"),
                "`approach` trigger `range` must be > 0 — set a positive block radius (e.g. 3)"
                    .to_string(),
            ));
        }
        if matches!(t.on, TriggerOn::Use)
            && let Some(at) = t.at_anchor()
            && let Some(npc) = c
                .npcs
                .content
                .npcs
                .iter()
                .find(|n| n.anchor.as_str() == at && n.offset == [0, 0, 0])
        {
            d.push(Diagnostic::error(
                USE_TRIGGER_ON_NPC,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "`use` trigger `{}` is anchored at `{}`, where NPC `{}` stands — a \
                     right-click there already belongs to the NPC's dialogue, and two \
                     interaction hitboxes in one cell race for the same click (the loser is \
                     silently dead, which can soft-lock the delve). Move the trigger to its \
                     own anchor, or express the interaction as a dialogue option on the NPC. \
                     (To make an NPC's body itself the target, use `strike-npc`.)",
                    t.id, at, npc.id
                ),
            ));
        }
        for (m, f) in t.requires_flags.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quests",
                    format!("/content/triggers/{i}/requires_flags/{m}"),
                    format!(
                        "trigger `requires_flags` references flag `{f}`, which no `set-flag` \
                         effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                         somewhere, or correct the flag name"
                    ),
                ));
            }
        }
        // v0.6: trigger-level `forbids_flags` — same unknown-flag treatment as
        // `requires_flags` (DW0172).
        for (m, f) in t.forbids_flags.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quests",
                    format!("/content/triggers/{i}/forbids_flags/{m}"),
                    format!(
                        "trigger `forbids_flags` references flag `{f}`, which no `set-flag` \
                         effect ever produces — the gate can never suppress anything; add the \
                         producing `set-flag {{ flag: \"{f}\" }}` effect, or correct the flag name"
                    ),
                ));
            }
        }
        for_each_trigger_effect_deep(t, |path, eff| {
            check_effect_v04(
                eff,
                blocks,
                &declared_waves,
                &format!("/content/triggers/{i}/{path}"),
                &npc_ids,
                d,
            );
        });
    }
}

// ---------------------------------------------------------------------------
// spec-0117: the pressable class — a real block, three states, an answer each
// ---------------------------------------------------------------------------

crate::dw_code! {
    /// (spec-0117) **A press that would be silent** in a state a player can
    /// press it in: a pressable with a shut or settled state (an `interact`, a
    /// gated, `once` or `serves` trigger, a shortcut, a disarm, a bonfire armed
    /// after the start) declares no line for that state, so a press there does
    /// nothing and says nothing. Also the silent block: `release` on a block the
    /// game reports no press of, which would leave scenery where a pressable
    /// stood. The engine never writes the words.
    pub const PRESS_SILENT: DwCode = DwCode::new("DW1051", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0117) **An answer for a state the press never has**, or a
    /// pressable field that binds to nothing: `missing_item` without
    /// `requires_item`; `not_yet` on a press with no shut state; `done` on one
    /// with no settled state, on a released one, or on a gate's own answer;
    /// `release` on a block pressed on its own or on a pressable with no settled
    /// state; `serves: {gate}` naming an anchor nothing seals; `prop` on a
    /// gate's own answer. An inert declaration is refused, never ignored.
    pub const PRESS_ANSWER_INERT: DwCode = DwCode::new("DW1052", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0117) **A press target that is not a real block**: an interaction
    /// point with no `prop` where one is required, a `prop` whose blockstate
    /// the pinned game gives no outline (or an outline that leaves its cell), a
    /// door named by its upper half, or a trigger anchored at a gate **region**
    /// (many cells; anchor it at the door's cell, where the gate's block is its
    /// body).
    pub const PRESS_TARGET_UNREAL: DwCode = DwCode::new("DW1059", ExitTier::Build);
}

/// **How a block is pressed** (spec-0117 §3a), read from the pinned game's own
/// report of a press on it (§2a). The one classification the validator and the
/// compiler both read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PressForm {
    /// Pressed **on its own**: the game reports the press (`default_block_use`
    /// at the cell) and it changes nothing the world keeps — a bell, a button,
    /// a lever on a pressable whose every press acts. Nothing is summoned.
    Own,
    /// Boxed because its press **acts on its own**: a lever with a shut or
    /// settled state, a door, a trapdoor, a fence gate, a container. The box
    /// stands outside the window; inside it the block acts by vanilla and its
    /// own report is the detector.
    ActsOnOwn,
    /// Boxed because the game reports **no** empty-hand press of it (a
    /// campfire, an iron door, stone), or reports one this engine does not read
    /// (a note block, a lectern): the box stands in every state and its record
    /// is the detector.
    Unobservable,
}

impl PressForm {
    /// The words a binding line and a ledger use.
    pub fn word(self) -> &'static str {
        match self {
            PressForm::Own => "own",
            PressForm::ActsOnOwn => "acts on its own",
            PressForm::Unobservable => "unobservable",
        }
    }

    /// Whether the block is pressed through a click box in some state.
    pub fn is_boxed(self) -> bool {
        self != PressForm::Own
    }
}

/// The form `block` is pressed in, or `None` when it is not a body at all: no
/// outline in the pinned table, or an outline that leaves its cell
/// (`DW1059`). `only_open` is true for a pressable whose every press acts (an
/// ungated, repeatable, unserved trigger; a shop) — the one place a lever is
/// pressed on its own.
///
/// **Cited** (spec-0117 §2a, the pinned jar): the bell's ring and the button's
/// pulse end by themselves; the lever keeps `powered`; a wooden or copper door,
/// every trapdoor and fence gate but the two iron ones open by hand; a chest and
/// every container open a screen and are reported whatever the screen does.
/// **Authored**: every other block with an outline is read as unobservable —
/// the box stands in every state — because the engine reads no empty-hand
/// report of it; the safe default, since a box over a block that would have
/// acted only withholds the act.
pub fn press_form(block: &str, only_open: bool) -> Option<PressForm> {
    if !outline_in_cell(block) {
        return None;
    }
    let id = crate::blockshape::bare_id(block);
    if id == "bell" || id.ends_with("_button") {
        return Some(PressForm::Own);
    }
    if id == "lever" {
        return Some(if only_open {
            PressForm::Own
        } else {
            PressForm::ActsOnOwn
        });
    }
    if crate::blockshape::is_player_openable(block)
        || crate::blocks::container_kind(block).is_some()
    {
        return Some(PressForm::ActsOnOwn);
    }
    Some(PressForm::Unobservable)
}

/// Whether `block` has an outline and every box of it lies within its own cell
/// (a door's within its two).
pub fn outline_in_cell(block: &str) -> bool {
    let Some(boxes) = crate::blockshape::outline(block) else {
        return false;
    };
    !boxes.is_empty()
        && boxes.iter().all(|b| {
            (0..3).all(|a| {
                let lo = b.min[a];
                let hi = b.max[a];
                lo.num >= 0 && i64::from(hi.num) <= 16 * i64::from(hi.den)
            })
        })
}

/// Whether a prop names a door by its upper half (`DW1059` (c)): a door is one
/// block in two cells and is named by its lower half.
pub fn names_upper_door_half(block: &str) -> bool {
    crate::blockshape::bare_id(block).ends_with("_door")
        && crate::blockshape::state_value(block, "half") == Some("upper")
}

/// The kind of interaction point a [`PressSite`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PressKind {
    /// An `interact` objective.
    Interact,
    /// A `use` trigger.
    Use,
    /// A `strike` at a place, a `strike-npc` or a `strike-assembly`.
    Strike,
    /// A `step` trigger: answers admitted, nothing owed (it is not a press).
    Step,
    /// A `shortcuts[]` unlock (and its sealed door's far-side line).
    Shortcut,
    /// A `bonfire` effect.
    Bonfire,
    /// A `shops[]` entry.
    Shop,
    /// A trap's `disarm`.
    TrapDisarm,
    /// A timed gate's `disarm`.
    TimedGateDisarm,
}

impl PressKind {
    /// The words a message uses.
    pub fn word(self) -> &'static str {
        match self {
            PressKind::Interact => "`interact` objective",
            PressKind::Use => "`use` trigger",
            PressKind::Strike => "strike trigger",
            PressKind::Step => "`step` trigger",
            PressKind::Shortcut => "shortcut",
            PressKind::Bonfire => "bonfire",
            PressKind::Shop => "shop",
            PressKind::TrapDisarm => "trap disarm",
            PressKind::TimedGateDisarm => "timed-gate disarm",
        }
    }
}

/// One interaction point the document declares (spec-0117 §3a), with the
/// states its press can be in — the object `DW1051`, `DW1052` and `DW1059`
/// judge, and the one list the compiler's pressables are built from.
#[derive(Clone, Debug)]
pub struct PressSite<'c> {
    /// What it is.
    pub kind: PressKind,
    /// Its id (`obj/…`, `trigger/…`, `shortcut/…`, the trap's or the gate's id,
    /// the bonfire's anchor).
    pub id: String,
    /// The JSON pointer the declaration sits at.
    pub path: String,
    /// The block, if declared.
    pub prop: Option<&'c Prop>,
    /// Whether a `prop` is owed here (`DW1059` (a)).
    pub prop_owed: bool,
    /// The declared answers, if any.
    pub answers: Option<&'c Answers>,
    /// `release`.
    pub release: bool,
    /// The press has a shut state (its window has not opened).
    pub shut: bool,
    /// The press has a settled state (its act is done).
    pub settled: bool,
    /// The press has a `missing_item` state (an `interact` with
    /// `requires_item`).
    pub missing_item: bool,
    /// Every press acts: the press has only the open state.
    pub only_open: bool,
    /// The trigger is a candidate gate's own answer: `serves: {gate}` and no
    /// `prop`. Its body is the gate's block; the build decides by geometry.
    pub gate_answer: bool,
    /// Is a strike (boxed in every state).
    pub struck: bool,
}

impl PressSite<'_> {
    /// The form its block is pressed in, when it names one.
    pub fn form(&self) -> Option<PressForm> {
        let prop = self.prop?;
        if self.struck {
            return outline_in_cell(&prop.block).then_some(PressForm::Unobservable);
        }
        press_form(&prop.block, self.only_open)
    }

    /// The answer states owed (`DW1051`), in [`AnswerState::ALL`] order.
    pub fn owed(&self) -> Vec<AnswerState> {
        if self.kind == PressKind::Step {
            return Vec::new();
        }
        let mut v = Vec::new();
        if self.shut {
            v.push(AnswerState::NotYet);
        }
        if self.missing_item {
            v.push(AnswerState::MissingItem);
        }
        if self.settled && !self.release && !self.gate_answer {
            v.push(AnswerState::Done);
        }
        v
    }

    /// The answer states the press can be in at all — a declared line for any
    /// other is inert (`DW1052`).
    pub fn admitted(&self) -> Vec<AnswerState> {
        let mut v = Vec::new();
        if self.shut {
            v.push(AnswerState::NotYet);
        }
        if self.missing_item {
            v.push(AnswerState::MissingItem);
        }
        if self.settled && !self.release && !self.gate_answer {
            v.push(AnswerState::Done);
        }
        v
    }
}

/// Whether a trigger's own gate can be shut: any of its three gate terms.
fn trigger_gated(t: &EnvTrigger) -> bool {
    !t.requires_flags.is_empty() || !t.forbids_flags.is_empty() || !t.requires_state.is_empty()
}

/// The gate anchors a `shortcut` seals from load.
fn shortcut_gates(c: &Campaign) -> BTreeSet<&str> {
    c.quests
        .content
        .shortcuts
        .iter()
        .map(|s| s.gate.as_str())
        .collect()
}

/// The gate anchors a `close-gate` seals, anywhere in the campaign.
pub(crate) fn close_gate_anchors(c: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |_path, _site, eff| {
        if let Some(a) = eff.close_gate_anchor() {
            out.insert(a.as_str().to_string());
        }
    });
    out
}

/// **Every interaction point the document declares** (spec-0117 §3a), with its
/// states: `interact` objectives, triggers (every kind but `approach`), shortcut
/// unlocks, bonfires, shops and both disarms. Deterministic: declaration order,
/// one fixed walk per section (ADR-0006).
pub fn press_sites(c: &Campaign) -> Vec<PressSite<'_>> {
    let quests = &c.quests.content;
    let sc_gates = shortcut_gates(c);
    let npc_stands: BTreeSet<&str> = c
        .npcs
        .content
        .npcs
        .iter()
        .filter(|n| n.offset == [0, 0, 0])
        .map(|n| n.anchor.as_str())
        .collect();
    let mut out = Vec::new();
    for (qi, q) in quests.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            let crate::Objective::Interact {
                id,
                prop,
                answers,
                release,
                requires_item,
                ..
            } = o
            else {
                continue;
            };
            out.push(PressSite {
                kind: PressKind::Interact,
                id: id.as_str().to_string(),
                path: format!("/content/quests/{qi}/objectives/{oi}"),
                prop: prop.as_ref(),
                prop_owed: true,
                answers: answers.as_ref(),
                release: *release,
                shut: true,
                settled: true,
                missing_item: requires_item.is_some(),
                only_open: false,
                gate_answer: false,
                struck: false,
            });
        }
    }
    for (ti, t) in quests.triggers.iter().enumerate() {
        let kind = match &t.on {
            TriggerOn::Use => PressKind::Use,
            TriggerOn::Strike | TriggerOn::StrikeNpc { .. } | TriggerOn::StrikeAssembly { .. } => {
                PressKind::Strike
            }
            TriggerOn::Step => PressKind::Step,
            TriggerOn::Approach { .. } => continue,
        };
        let (serves_shut, serves_settled) = match &t.serves {
            None => (false, false),
            Some(Serves::Objective(_)) => (true, true),
            // A shortcut's gate is sealed from load: no shut state.
            Some(Serves::Gate(g)) => (!sc_gates.contains(g.as_str()), true),
        };
        let gate_answer = matches!(t.serves, Some(Serves::Gate(_)))
            && t.prop.is_none()
            && matches!(t.on, TriggerOn::Use);
        let at_point = matches!(t.on, TriggerOn::Use | TriggerOn::Strike)
            && t.at_anchor()
                .is_some_and(|a| !(matches!(t.on, TriggerOn::Strike) && npc_stands.contains(a)));
        let shut = trigger_gated(t) || serves_shut;
        let settled = t.once || serves_settled;
        out.push(PressSite {
            kind,
            id: t.id.as_str().to_string(),
            path: format!("/content/triggers/{ti}"),
            prop: t.prop.as_ref(),
            prop_owed: at_point && !gate_answer,
            answers: t.answers.as_ref(),
            release: t.release,
            // A gate's own answer has a body only while the gate is sealed.
            shut: shut && !gate_answer,
            settled: settled && !gate_answer,
            missing_item: false,
            only_open: !shut && !settled,
            gate_answer,
            struck: matches!(t.on, TriggerOn::Strike) && at_point,
        });
    }
    for (si, sc) in quests.shortcuts.iter().enumerate() {
        out.push(PressSite {
            kind: PressKind::Shortcut,
            id: sc.id.as_str().to_string(),
            path: format!("/content/shortcuts/{si}"),
            prop: sc.prop.as_ref(),
            prop_owed: true,
            answers: sc.answers.as_ref(),
            release: sc.release,
            // The sealed door's far-side line is the shortcut's `not_yet`.
            shut: true,
            settled: true,
            missing_item: false,
            only_open: false,
            gate_answer: false,
            struck: false,
        });
    }
    let mut bonfires = Vec::new();
    crate::for_each_campaign_effect(c, &mut |path, _site, eff| {
        if let crate::Verb::Bonfire {
            anchor,
            prop,
            answers,
            ..
        } = &eff.verb
        {
            bonfires.push(PressSite {
                kind: PressKind::Bonfire,
                id: anchor.as_str().to_string(),
                path: path.to_string(),
                prop: prop.as_ref(),
                prop_owed: true,
                answers: answers.as_ref(),
                release: false,
                // Every root arms its bonfire on an event after the world
                // loads, so a bonfire always has a shut state.
                shut: true,
                settled: false,
                missing_item: false,
                only_open: false,
                gate_answer: false,
                struck: false,
            });
        }
    });
    out.extend(bonfires);
    for (hi, sh) in quests.shops.iter().enumerate() {
        out.push(PressSite {
            kind: PressKind::Shop,
            id: sh.id.as_str().to_string(),
            path: format!("/content/shops/{hi}"),
            prop: sh.prop.as_ref(),
            prop_owed: true,
            answers: None,
            release: false,
            shut: false,
            settled: false,
            missing_item: false,
            only_open: true,
            gate_answer: false,
            struck: false,
        });
    }
    for (ti, t) in quests.traps.iter().enumerate() {
        let Some(d) = &t.disarm else { continue };
        out.push(PressSite {
            kind: PressKind::TrapDisarm,
            id: t.id.as_str().to_string(),
            path: format!("/content/traps/{ti}/disarm"),
            prop: d.prop.as_ref(),
            prop_owed: true,
            answers: d.answers.as_ref(),
            release: d.release,
            shut: false,
            settled: true,
            missing_item: false,
            only_open: false,
            gate_answer: false,
            struck: false,
        });
    }
    for (gi, g) in quests.timed_gates.iter().enumerate() {
        let Some(d) = &g.disarm else { continue };
        out.push(PressSite {
            kind: PressKind::TimedGateDisarm,
            id: g.id.as_str().to_string(),
            path: format!("/content/timed_gates/{gi}/disarm"),
            prop: d.prop.as_ref(),
            prop_owed: true,
            answers: d.answers.as_ref(),
            release: d.release,
            shut: false,
            settled: true,
            missing_item: false,
            only_open: false,
            gate_answer: false,
            struck: false,
        });
    }
    out
}

/// What `DW1051`, `DW1052` and `DW1059` examined, stated whichever way they
/// went.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PressBinding {
    /// Interaction points examined (steps excluded).
    pub pressables: usize,
    /// Of them, pressed on their own, by kind: bells, buttons, levers.
    pub own_bell: usize,
    /// Buttons pressed on their own.
    pub own_button: usize,
    /// Levers pressed on their own.
    pub own_lever: usize,
    /// Boxed because the press acts on its own.
    pub boxed_acts: usize,
    /// Boxed because the press is unobservable.
    pub boxed_unobservable: usize,
    /// Boxed because the press is a strike.
    pub boxed_struck: usize,
    /// Riding a body (a `strike-npc`, a `strike-assembly`, a strike at an
    /// NPC's stand) or a gate's own answer: no prop of their own.
    pub riding: usize,
    /// Answer lines owed.
    pub owed: usize,
    /// Answer lines declared.
    pub declared: usize,
    /// Pressables refused by `DW1059`.
    pub refused_target: usize,
    /// Lines refused by `DW1051` (missing) or `DW1052` (inert).
    pub refused_answers: usize,
    /// Candidate gate's own answers, judged at the build by `DW0429`.
    pub gate_answers: usize,
    /// Released pressables.
    pub released: usize,
    /// Pressables with only the open state, owing nothing.
    pub only_open: usize,
}

impl PressBinding {
    /// `DW1059`'s binding line.
    pub fn bodies_line(&self) -> String {
        format!(
            "press bodies (DW1059): {} interaction point(s) — {} pressed on their own ({} bell, \
             {} button, {} lever), {} boxed ({} acting on their own, {} unobservable, {} \
             struck), {} riding a body or a gate, {} refused",
            self.pressables,
            self.own_bell + self.own_button + self.own_lever,
            self.own_bell,
            self.own_button,
            self.own_lever,
            self.boxed_acts + self.boxed_unobservable + self.boxed_struck,
            self.boxed_acts,
            self.boxed_unobservable,
            self.boxed_struck,
            self.riding,
            self.refused_target
        )
    }

    /// `DW1051`'s binding line.
    pub fn answers_line(&self) -> String {
        format!(
            "press answers (DW1051): {} pressable(s) — {} answer(s) owed, {} declared, {} \
             refused; {} gate answer(s) judged by DW0429; {} released; {} with only the open \
             state",
            self.pressables,
            self.owed,
            self.declared,
            self.refused_answers,
            self.gate_answers,
            self.released,
            self.only_open
        )
    }
}

/// `DW1051`, `DW1052`, `DW1059` over every interaction point of the primary
/// document (spec-0117 §6). The arms that need the placed world — a trigger
/// anchored at a gate region, a gate's own answer that is really a knocker, a
/// `prop` on a gate's own answer — are the build's (`compiler::press`).
pub fn press_checks(c: &Campaign) -> (Vec<Diagnostic>, PressBinding) {
    let mut d = Vec::new();
    let mut b = PressBinding::default();
    let ambush_ids: BTreeMap<String, usize> = c
        .quests
        .content
        .ambushes
        .iter()
        .enumerate()
        .map(|(i, a)| {
            (
                format!("trigger/{}", crate::l10n::local_id(a.id.as_str())),
                i,
            )
        })
        .collect();
    let sealed_by_close = close_gate_anchors(c);
    let sc_gates = shortcut_gates(c);
    for site in press_sites(c) {
        if site.kind != PressKind::Step {
            b.pressables += 1;
        }
        if site.gate_answer {
            b.gate_answers += 1;
        }
        if site.release {
            b.released += 1;
        }
        if site.only_open && site.kind != PressKind::Step {
            b.only_open += 1;
        }
        // --- DW1059: the body is a real block ---
        let mut target_ok = true;
        match site.prop {
            None if site.prop_owed => {
                target_ok = false;
                let (path, remedy) = match ambush_ids.get(&site.id) {
                    Some(ai) => (
                        format!("/content/ambushes/{ai}/trigger"),
                        "an ambush carries no block to press: spring it on an `approach`, or \
                         write the trigger yourself with the `prop` the party presses"
                            .to_string(),
                    ),
                    None => (
                        format!("{}/prop", site.path),
                        format!(
                            "declare `\"prop\": {{\"block\": \"<the block the party presses>\"}}` — \
                             a bell or a button is pressed as itself; a door, a lever, a chest, a \
                             campfire or any other block gets an invisible box fitted to it{}",
                            if site.kind == PressKind::Strike {
                                "; a strike is always boxed, because the game reports no \
                                 left-click on a block"
                            } else {
                                ""
                            }
                        ),
                    ),
                };
                d.push(Diagnostic::error(
                    PRESS_TARGET_UNREAL,
                    "quests",
                    path,
                    format!(
                        "{} `{}` asks the party to press a thing and names no block for it: a \
                         press target is a real block the player sees, drawn and occluded like \
                         the world, and an invisible box in the air is nothing a player can \
                         find (arm a). Prescription: {remedy}",
                        site.kind.word(),
                        site.id
                    ),
                ));
            }
            None => {
                if site.kind != PressKind::Step {
                    b.riding += 1;
                }
            }
            Some(p) if names_upper_door_half(&p.block) => {
                target_ok = false;
                d.push(Diagnostic::error(
                    PRESS_TARGET_UNREAL,
                    "quests",
                    format!("{}/prop/block", site.path),
                    format!(
                        "{} `{}` names the door `{}` by its upper half (arm c). A door is one \
                         block in two cells and is named by its lower half: the compiler places \
                         both halves and fits one box over the two cells. Prescription: write \
                         `half=lower`, or leave `half` out",
                        site.kind.word(),
                        site.id,
                        p.block
                    ),
                ));
            }
            Some(p) if site.form().is_none() => {
                target_ok = false;
                let why = if crate::blockshape::has_outline(&p.block) {
                    "its outline reaches outside its own cell, so no box fitted to it can stay in \
                     the cell it stands in"
                } else {
                    "the pinned game gives that blockstate no outline — nothing a crosshair can \
                     land on (air, a fluid, `light`)"
                };
                d.push(Diagnostic::error(
                    PRESS_TARGET_UNREAL,
                    "quests",
                    format!("{}/prop/block", site.path),
                    format!(
                        "{} `{}` declares the prop `{}`, and {why} (arm b). A click box may stand \
                         over a real block only, never in the air. Prescription: name a block \
                         that reads as the thing — a lever, a bell, a door, a chest, a campfire",
                        site.kind.word(),
                        site.id,
                        p.block
                    ),
                ));
            }
            Some(p) => match site.form() {
                Some(PressForm::Own) => match crate::blockshape::bare_id(&p.block) {
                    "bell" => b.own_bell += 1,
                    "lever" => b.own_lever += 1,
                    _ => b.own_button += 1,
                },
                Some(_) if site.struck => b.boxed_struck += 1,
                Some(PressForm::ActsOnOwn) => b.boxed_acts += 1,
                Some(PressForm::Unobservable) => b.boxed_unobservable += 1,
                None => {}
            },
        }
        if !target_ok {
            b.refused_target += 1;
        }
        // --- DW1052: inert fields ---
        let mut inert_fields: Vec<(String, String)> = Vec::new();
        let mut inert = |field: &str, why: String| inert_fields.push((field.to_string(), why));
        let mut refused_here = 0usize;
        if let Some(a) = site.answers {
            for (state, _) in a.declared() {
                if site.admitted().contains(&state) {
                    continue;
                }
                refused_here += 1;
                let why = match state {
                    AnswerState::NotYet if site.gate_answer => {
                        "this is the gate's own answer: its body exists only while the gate is \
                         sealed, so it is never pressed before"
                            .to_string()
                    }
                    AnswerState::NotYet => {
                        "the press has no shut state — nothing gates it, so it is open from the \
                         start"
                            .to_string()
                    }
                    AnswerState::MissingItem => {
                        "no `requires_item` is declared, so the hand is never short of an item"
                            .to_string()
                    }
                    AnswerState::Done if site.release => {
                        "the press is released: at settlement the block is vanilla's, and \
                         opening is the answer"
                            .to_string()
                    }
                    AnswerState::Done if site.gate_answer => {
                        "this is the gate's own answer: its block is filled to air with the \
                         gate, so nothing stands to be pressed after"
                            .to_string()
                    }
                    AnswerState::Done => {
                        "the press never settles — it is not `once` and serves nothing".to_string()
                    }
                };
                inert(&format!("answers/{}", state.field()), why);
            }
        }
        if site.release {
            if !site.settled {
                refused_here += 1;
                inert(
                    "release",
                    "the press has no settled state for the block to be released at".to_string(),
                );
            } else if site.form() == Some(PressForm::Own) {
                refused_here += 1;
                inert(
                    "release",
                    "the block is pressed on its own and was never boxed, so there is nothing to \
                     release"
                        .to_string(),
                );
            } else if site.form() == Some(PressForm::Unobservable) && !site.struck {
                refused_here += 1;
                d.push(Diagnostic::error(
                    PRESS_SILENT,
                    "quests",
                    format!("{}/release", site.path),
                    format!(
                        "{} `{}` releases its block at settlement, and the pinned game reports no \
                         press of `{}`: the released block would answer nothing and no line could \
                         be written for it — scenery where a pressable stood. Prescription: drop \
                         `release` and write `answers.done`",
                        site.kind.word(),
                        site.id,
                        site.prop.map_or("", |p| p.block.as_str())
                    ),
                ));
            }
        }
        if site.kind == PressKind::Use
            && let Some(Serves::Gate(g)) = c
                .quests
                .content
                .triggers
                .iter()
                .find(|t| t.id.as_str() == site.id)
                .and_then(|t| t.serves.as_ref())
            && !sealed_by_close.contains(g.as_str())
            && !sc_gates.contains(g.as_str())
        {
            refused_here += 1;
            inert(
                "serves/gate",
                format!(
                    "`{g}` is sealed by no `close-gate` and is no shortcut's gate, so the press \
                     serves nothing"
                ),
            );
        }
        for (field, why) in inert_fields {
            d.push(Diagnostic::error(
                PRESS_ANSWER_INERT,
                "quests",
                format!("{}/{field}", site.path),
                format!(
                    "{} `{}` declares `{field}`, and {why}. An inert declaration is refused, \
                     never ignored. Prescription: drop it, or give the press the gate, `once`, \
                     `serves` or `requires_item` that creates the state",
                    site.kind.word(),
                    site.id
                ),
            ));
        }
        // --- DW1051: silence ---
        let owed = site.owed();
        b.owed += owed.len();
        b.declared += site.answers.map_or(0, |a| a.declared().len());
        for state in owed {
            if site.answers.and_then(|a| a.line(state)).is_some() {
                continue;
            }
            refused_here += 1;
            let meet = match state {
                AnswerState::NotYet if site.kind == PressKind::Shortcut => {
                    "a player on the far side presses the barred door before the bar is thrown"
                }
                AnswerState::NotYet => "a player presses it before its step",
                AnswerState::MissingItem => {
                    "a player presses it at its step without the item in hand"
                }
                AnswerState::Done => "a player presses it after it is done",
            };
            d.push(Diagnostic::error(
                PRESS_SILENT,
                "quests",
                format!("{}/answers/{}", site.path, state.field()),
                format!(
                    "{} `{}` would be silent: {meet}, and the press does nothing and says \
                     nothing. Prescription: write `answers.{}` — one line, in the campaign's \
                     own words, saying why nothing happened; it is shown on the presser's \
                     action bar with the refusal cue",
                    site.kind.word(),
                    site.id,
                    state.field()
                ),
            ));
        }
        b.refused_answers += refused_here;
    }
    (d, b)
}
