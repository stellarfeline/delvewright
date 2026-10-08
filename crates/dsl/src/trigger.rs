//! Environment triggers and the props a player interacts with.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::default_true;
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
    /// **The visible object the click acts on** (spec-0093 §6.5): for a `use`
    /// or a `strike` trigger, the block the compiler places at `at` — a lever,
    /// a bell, a lamp, a stone. A `use` on a block vanilla reports the use of
    /// (a lever, a button, a bell) fires through vanilla's `default_block_use`
    /// criterion and summons no hitbox; any other prop, and every `strike`, is
    /// placed with the `minecraft:interaction` hitbox fitted over it as its hit
    /// area. A click trigger with no `prop` on open air is refused (`DW0963`):
    /// the hitbox is invisible and is never the object. A `prop` on an event
    /// with no cell of its own — an approach, a `strike-npc`, a
    /// `strike-assembly` — is `DW0964`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prop: Option<Prop>,
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
    /// empty, so a pre-0.10 campaign is byte-identical.
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
    /// `minecraft:player_interacted_with_entity`) and a step (`step`, a player
    /// in the cell). A left-click is recorded as a UUID no command can become;
    /// everything else is refused by `DW0427`.
    pub fn attributes_its_actor(&self) -> bool {
        matches!(self.on, TriggerOn::Use | TriggerOn::Step)
    }
}

/// Who an [`EnvTrigger`]'s effects address (DSL v0.11).
///
/// **This is a dispatch decision, not a cosmetic one.** A `party` trigger is
/// polled on the tick with no executor, so `@s` does not exist and every
/// player-facing command addresses `@a`. A `presser` trigger is dispatched by a
/// `minecraft:player_interacted_with_entity` advancement — the one vanilla
/// primitive that runs a function *as the player who clicked* — so `@s` is the
/// presser and the bundle addresses them alone. A `presser` trigger `on: step`
/// is polled as `execute as @a[<the cell>]`, so `@s` is each player who stepped
/// on, on their own step.
///
/// The click primitive exists for **right-clicks only**. Vanilla records a
/// left-click on an interaction entity in NBT (which names a UUID no command can
/// become) and offers no criterion for it, so `presser` on a `strike` is refused
/// (`DW0427`) rather than approximated: per CLAUDE.md's no-hack rule, a
/// capability with no vanilla primitive under it is excluded, never faked
/// downstream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TriggerAudience {
    /// The whole party (the default, and what every trigger did before v0.11).
    #[default]
    Party,
    /// The one player whose press fired it: the right-click of a `use`, the
    /// step of a `step`.
    Presser,
}

impl TriggerAudience {
    /// Serde skip predicate: the default needs no field on the wire, so a
    /// canonical round-trip of a pre-0.11 campaign is byte-identical.
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

/// A prop block for an `interact` objective or a `use` trigger (DSL v0.4;
/// spec-0093 §6.5). The block is the affordance the player interacts with; its
/// id is validated against the pinned 1.21.11 block registry (`DW0193`).
///
/// **When the block is one a hand presses** — a lever or a button
/// ([`crate::blockshape::is_hand_pressed`]) — the block IS the detector: the
/// compiler summons no `minecraft:interaction` hitbox and the act is vanilla's
/// own, reported by the `default_block_use` advancement criterion at the
/// block's cell. Any other block is placed and the invisible hitbox stands in
/// its cell, because vanilla reports no use of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Prop {
    /// Vanilla block id, with an optional blockstate suffix (e.g.
    /// `minecraft:lever[face=floor,facing=north]`).
    pub block: String,
}

impl Prop {
    /// Whether this prop's block is one a hand presses — the block is then the
    /// act's own detector (spec-0093 §6.5).
    pub fn is_hand_pressed(&self) -> bool {
        crate::blockshape::is_hand_pressed(&self.block)
    }
}
