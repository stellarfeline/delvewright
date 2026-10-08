//! Traps: what fires them, what they do and how they reset or are disarmed.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AnchorId, FlagId, QuestEffect, StateCompare, TrapId};

#[cfg(doc)]
use crate::EnvTrigger;

/// A stage-5 trap (DSL v0.6, spec-0011; command payloads spec-0022): an
/// environmental hazard at one cell of a placed piece.
///
/// **What the prefab has to provide is one point anchor with the trigger block
/// in its cell, and for most traps that is all.** [`Trap::at`] names the
/// trigger/hazard cell; the piece places the plate, tripwire or trapped chest
/// there (`DW0917`); the compiler models it as a hazard for the completability
/// proofs (`DW0342`) and, for a disarmable trap, emits the disarm affordance. A
/// [`payload`](Trap::payload) trap needs nothing else: **the compiler owns the
/// detection**, emitting a per-tick, edge-latched `execute … if entity @a[<cell>]` and running the authored effect
/// bundle from it.
///
/// Two things a piece must pre-wire, each for one case and neither for the
/// common one:
///
/// * the legacy [`effect`](Trap::effect) — a `dispense` payload the prefab's own
///   redstone fires — needs the anchor's `dispenser` socket cell, which the
///   compiler fills. That is the case "harm is redstone-native" (spec-0011) was
///   written about, and the only one in which no detection is emitted.
/// * a **flag-gated** trap ([`requires_flags`](Trap::requires_flags) /
///   [`forbids_flags`](Trap::forbids_flags)) needs the anchor's `trigger_block`,
///   because gating removes the trigger block from the world while the gate is
///   shut and puts it back verbatim (`DW0363`).
///
/// Player-vs-mob distinguishing matters in a sealed box-garden with controlled
/// mobs, so `trapped-chest` (opened by a player) is called out as the only
/// player-distinct trigger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Trap {
    /// Unique trap id (`trap/<kebab>`).
    pub id: TrapId,
    /// **The point anchor this trap sits on** — any anchor an area's prefab
    /// provides, whatever it is called. Its cell is the trigger/hazard cell the
    /// compiler models, and for a `payload` trap that cell, holding the block
    /// its [`trigger`](Trap::trigger) names, is the whole of what the piece has
    /// to provide: detection is the compiler's, the block is the piece's
    /// (`DW0917`).
    ///
    /// The anchor additionally needs a `dispenser` socket for a legacy
    /// [`effect`](Trap::effect) trap, and a `trigger_block` for a flag-gated one
    /// (`DW0363`). `anchor/trap` is the name the shipped pieces use, and a name
    /// is all it is.
    pub at: AnchorId,
    /// What springs the trap (all redstone-native).
    pub trigger: TrapTrigger,
    /// The **legacy** redstone consequence (spec-0011): a static dispenser
    /// payload the prefab's own wiring fires. Superseded by [`Trap::payload`]
    /// (spec-0022) — redstone now keeps only the trigger — but kept meaningful
    /// so existing campaigns build unchanged. Optional since spec-0022; a trap
    /// must declare `effect`, `payload`, or both (`DW0440`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<TrapEffect>,
    /// The **command payload** (spec-0022): an ordered effect list in the same
    /// vocabulary quests use, run when the trigger fires. This is where a trap's
    /// consequence lives now — the compiler owns the detection tick and the
    /// effect vocabulary, so a trap's payload is authored like any other effect
    /// bundle rather than built out of dust and repeaters. Expressiveness moves
    /// from "what dust can carry" to "what the effect vocabulary can say":
    /// `volley` and `collapse` (spec-0022's trap verbs) join `damage-players`,
    /// `play-sound`, `narrate`, `set-flag` and `spawn-wave`.
    ///
    /// Empty = a pure spec-0011 redstone trap, which emits exactly what it
    /// emitted before (byte-identical).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payload: Vec<QuestEffect>,
    /// How dangerous the trap is. A `lethal` trap on the forced critical path
    /// carries the completability obligation (`DW0342`); `harmful`/`nonlethal`
    /// carry none. Defaults to `harmful`.
    #[serde(default)]
    pub lethality: Lethality,
    /// Optional disarm affordance (quest-coupling): an anchor the player acts on to
    /// turn the trap off — setting a flag and emptying the dispenser — before the
    /// trap cell is forced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disarm: Option<TrapDisarm>,
    /// Whether the trap re-arms after firing. `once` = single-shot (fires, then
    /// spent — the survivability path); `rearm` = re-fires each trigger (default).
    #[serde(default)]
    pub reset: TrapReset,
    /// Flags that must be set before the trap is considered active (mirrors
    /// [`EnvTrigger::requires_flags`]). Default empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Negative flag gate (DSL v0.6): the trap is considered inactive while ANY
    /// listed flag is set (mirrors [`EnvTrigger::forbids_flags`]). Default empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Numeric gate terms (DSL v0.10, spec-0031): every listed comparison must
    /// hold for this gate to be open. The third field of the one gate, carried by
    /// every gate consumer — never by the verb that first wanted it. Default
    /// empty, so a pre-0.10 campaign is byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_state: Vec<StateCompare>,
}

impl Trap {
    /// `(item, count)` if this trap declares a legacy `dispense` effect.
    pub fn dispense(&self) -> Option<(&str, u32)> {
        match &self.effect {
            Some(TrapEffect::Dispense { item, count }) => Some((item.as_str(), *count)),
            None => None,
        }
    }

    /// Whether this trap is `lethal` (carries the `DW0342` obligation on the
    /// forced critical path).
    pub fn is_lethal(&self) -> bool {
        matches!(self.lethality, Lethality::Lethal)
    }
}

/// The mechanism that springs a [`Trap`] (DSL v0.6, spec-0011). All three are
/// redstone-native — the hardware fires without any command — so the compiler
/// emits no detection for them; it only models the trigger cell as a hazard and
/// fills the dispenser payload. (`approach`, the compiler-detected v0.4 primitive,
/// is deliberately *not* a trap trigger: it is already fully expressible as an
/// [`EnvTrigger`], so admitting it here would only duplicate that surface.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TrapTrigger {
    /// A pressure plate: any entity stepping on the cell. Auto-rearms on step-off.
    PressurePlate,
    /// A tripwire line: any entity crossing it. Auto-rearms.
    Tripwire,
    /// A trapped chest: a *player opening it* (comparator pulse). The only
    /// player-distinct trigger — a controlled mob cannot spring it.
    TrappedChest,
}

impl TrapTrigger {
    /// The kebab tag (`pressure-plate` / `tripwire` / `trapped-chest`).
    pub fn kind(&self) -> &'static str {
        match self {
            TrapTrigger::PressurePlate => "pressure-plate",
            TrapTrigger::Tripwire => "tripwire",
            TrapTrigger::TrappedChest => "trapped-chest",
        }
    }

    /// Whether `block` (an id, with or without its blockstate) is the hardware
    /// this trigger kind names: the block the party sees and springs. A plate
    /// is any `*_pressure_plate`, a tripwire is the string itself
    /// (`minecraft:tripwire`, not the hook), a trapped chest is
    /// `minecraft:trapped_chest`.
    pub fn is_trigger_block(&self, block: &str) -> bool {
        let id = block.split('[').next().unwrap_or(block);
        match self {
            TrapTrigger::PressurePlate => id.ends_with("_pressure_plate"),
            TrapTrigger::Tripwire => id == "minecraft:tripwire",
            TrapTrigger::TrappedChest => id == "minecraft:trapped_chest",
        }
    }

    /// The block [`TrapTrigger::is_trigger_block`] accepts, as a refusal names it.
    pub fn trigger_block_name(&self) -> &'static str {
        match self {
            TrapTrigger::PressurePlate => "a pressure plate (`minecraft:*_pressure_plate`)",
            TrapTrigger::Tripwire => "a tripwire string (`minecraft:tripwire`)",
            TrapTrigger::TrappedChest => "a trapped chest (`minecraft:trapped_chest`)",
        }
    }
}

impl TrapTrigger {
    /// The trigger kinds a body fires by walking onto them — a plate and a
    /// tripwire. A trapped chest is opened, not stepped on.
    pub const STEPPED: [TrapTrigger; 2] = [TrapTrigger::PressurePlate, TrapTrigger::Tripwire];
}

/// **Every block of the pinned registry that a step fires**, sorted: the
/// registry's ids that [`TrapTrigger::is_trigger_block`] accepts for a
/// [`TrapTrigger::STEPPED`] kind. Read from `crates/dsl/data/blocks-1.21.11.json`
/// rather than listed, so a pin that adds a plate adds it here;
/// `crates/delvec/tests/stepped_blocks_tag.rs` holds the set equal to vanilla's
/// own `#pressure_plates` tag plus the tripwire string.
pub fn stepped_blocks() -> Vec<&'static str> {
    crate::blocks::BlockRegistry::v1_21_11()
        .ids()
        .filter(|id| TrapTrigger::STEPPED.iter().any(|k| k.is_trigger_block(id)))
        .collect()
}

/// Whether `block` — an id, bare or namespaced, with or without a blockstate —
/// is one a step fires ([`stepped_blocks`]).
pub fn fires_on_step(block: &str) -> bool {
    let id = block.split('[').next().unwrap_or(block);
    let id = if id.contains(':') {
        id.to_string()
    } else {
        format!("minecraft:{id}")
    };
    stepped_blocks().contains(&id.as_str())
}

/// What a [`Trap`] does when sprung (DSL v0.6, spec-0011). Externally tagged so a
/// future effect adds a variant; a non-`dispense` key (e.g. `tnt`,
/// `release-falling-block`, `crusher`) is an unknown variant → `DW0100`, keeping
/// block-destroying and unmodeled effects out of the schema by construction
/// (spec-0011 non-goals — no hardware the compiler cannot model reaches a world).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum TrapEffect {
    /// Load the prefab's pre-wired dispenser with `count` of `item` (arrows, tipped
    /// arrows, splash potions). The redstone fires it; terrain is untouched
    /// (spec-0011 "primary lethal"). The item is round-tripped into the dispenser
    /// `Items` NBT — a deterministic, static payload.
    Dispense {
        /// Vanilla item id (validated against the pinned 1.21.11 registry, `DW0341`).
        item: String,
        /// How many to load into the dispenser stack.
        count: u32,
    },
}

/// How dangerous a [`Trap`] is (DSL v0.6, spec-0011). Only `lethal` carries the
/// forced-critical-path completability obligation (`DW0342`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Lethality {
    /// Can kill a full-health player — carries the `DW0342` obligation on the path.
    Lethal,
    /// Hurts but is not designed to kill (the default).
    #[default]
    Harmful,
    /// Cosmetic / trivial (a stumble, a scare).
    Nonlethal,
}

/// Whether a [`Trap`] re-arms after firing (DSL v0.6, spec-0011).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TrapReset {
    /// Fires once, then is spent — the survivability path for a forced lethal trap
    /// (respawn-safe with `keep_inventory`, non-re-triggering on the walk back; no
    /// soft-loop).
    Once,
    /// Re-fires every time the trigger is met (default). A forced lethal `rearm`
    /// trap must be avoidable or disarmable, else `DW0342`.
    #[default]
    Rearm,
}

/// A [`Trap`]'s disarm affordance (DSL v0.6, spec-0011): the player acts on the
/// `via` anchor (an interaction the compiler emits, reusing the v0.4 interaction
/// entity) to turn the trap off — setting `sets_flag` and emptying the dispenser —
/// before the trap cell is forced. Discharges the `DW0342` obligation when the
/// affordance is reachable ahead of the trap without crossing it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrapDisarm {
    /// The anchor the player interacts with to disarm.
    pub via: AnchorId,
    /// The flag set when the trap is disarmed (a new flag this trap produces; other
    /// objectives/triggers may read it via `requires_flags`).
    pub sets_flag: FlagId,
}
