//! Timed gates: a door that closes on a clock once opened.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::{is_false, is_zero};
use crate::{AnchorId, FlagId, TimedGateId};

#[cfg(doc)]
use crate::TrapDisarm;

/// A stage-5 **timed gate** (spec-0016 §4): a gate region driven by a
/// deterministic open/close clock, so passage is a timing read rather than a
/// permanent state.
///
/// **The proof is deliberately NOT all-phase passability** — a gate that
/// punishes bad timing is the entire point. What the
/// compiler requires is that the gate is *readable*: the set of entry phases from
/// which a walking player clears the span before it shuts must cover **≥ 20% of
/// the cycle** (`DW0378`). Below that it is a coin flip, not a skill, and no
/// amount of learning the level makes it fair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimedGate {
    /// Unique timed-gate id (`timed-gate/<kebab>`).
    pub id: TimedGateId,
    /// The gate anchor the clock drives. Its prefab metadata must declare a fill
    /// `block` (`DW0343`), the same requirement `close-gate` and `shortcut` have.
    pub gate: AnchorId,
    /// Ticks the gate stays OPEN each cycle (> 0).
    pub open_ticks: u32,
    /// Ticks the gate stays CLOSED each cycle (> 0).
    pub closed_ticks: u32,
    /// Ticks after world init before the first open window begins (default 0) —
    /// how two gates in the same room are put out of step with each other. Must
    /// be less than the full cycle.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub phase: u32,
    /// Whether the gate **kills** a player caught inside its region when it
    /// shuts (spec-0016 §4 addendum). A portcullis
    /// that merely shoves you aside teaches nothing; mistiming the crossing is
    /// supposed to be a death you learn from, which is the whole point of §4's
    /// ≥20%-of-cycle window proof — the window is fair, so the penalty can be
    /// absolute.
    ///
    /// This is a real judgement issued by command on the closing tick, not
    /// suffocation: vanilla's in-wall damage is slow, gear-dependent and
    /// escapable, so it would make the portcullis a suggestion. `damage` at the
    /// closing edge is exact and unarguable.
    ///
    /// **Defaults to `false`**, so every campaign authored before this field
    /// existed compiles byte-identically; a delve opts its portcullis in.
    #[serde(default, skip_serializing_if = "is_false")]
    pub crush: bool,
    /// Optional **disarm** affordance (souls dossier §5.2): the third
    /// rung of the hazard ladder — readable, avoidable, and finally *disable-able*.
    /// The real games' best timed hazards can be removed for good (Smouldering
    /// Lake's ballista, the Fringefolk chariot); a clock the party can only ever
    /// dance with is one rung short of the vocabulary.
    ///
    /// Interacting with the affordance suppresses the clock **permanently, with
    /// the gate resting OPEN** — a jammed portcullis stays up. Permanence is
    /// structural exactly as a `shortcut`'s is: no emitted function ever re-arms
    /// the clock, and `DW0389` refuses a campaign that spells a re-seal.
    ///
    /// **Defaults to absent**, so every campaign authored before this field
    /// existed compiles byte-identically; a delve opts its portcullis in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disarm: Option<TimedGateDisarm>,
}

/// A [`TimedGate`]'s disarm affordance (souls dossier §5.2) — the
/// exact shape a trap's [`TrapDisarm`] takes, and deliberately so: one affordance
/// grammar for every mechanism the party can switch off.
///
/// The player acts on the `via` anchor (a compiler-emitted interaction entity
/// plus its visible hardware, `DW0420`) to jam the gate. The clock stops with the
/// span cleared, `sets_flag` is raised party-wide, and nothing in the delve can
/// put the gate back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimedGateDisarm {
    /// The anchor the player interacts with to jam the gate. Must be an anchor
    /// some area's prefab provides, and never the gate anchor itself — the
    /// mechanism belongs beside the doorway, not inside the span that crushes.
    pub via: AnchorId,
    /// The flag set when the gate is disarmed (a new flag this gate produces;
    /// other objectives/triggers may read it via `requires_flags`).
    pub sets_flag: FlagId,
}
