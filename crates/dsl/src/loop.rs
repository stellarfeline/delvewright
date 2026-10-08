//! Loops: an endless corridor (spec-0086).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{FlagId, LoopId, Mark, QuestEffect, StateCompare, StateId, StealthZone};

#[cfg(doc)]
use crate::LethalVolume;

/// A stage-5 **loop** (spec-0086): a slab a body crosses and is returned from,
/// by a whole-block offset, to an earlier section that looks exactly the same —
/// its position inside the cell, its facing and its velocity all kept.
///
/// # It is a region with a standing property
///
/// A loop acts on whatever body enters a volume, every tick, the way a
/// [`LethalVolume`] does; nothing completes and nobody is addressed. So it is
/// declared beside the lethal volume, with the same region type
/// ([`StealthZone`], resolved through the one `Plan::zone_box`), and not as a
/// trigger with a relative teleport in it: the seamlessness proof is a property
/// of the region-plus-offset pair, which a compiler would otherwise have to
/// recognise by pattern-matching a trigger's effect list.
///
/// # The offset is derived, never typed
///
/// `to` is a [`Mark`] naming where the slab's own anchor cell lands, so the
/// offset is `cell(to) − cell(region.anchor)` — a whole-block vector by
/// construction, and a judgement about the world (*the fourth bay's anchor lands
/// on the second's*) rather than a vector the compiler could compute.
///
/// # The gate is the release
///
/// The loop **holds** while its gate is open and stands down while it is shut,
/// read against the party: flags are campaign state, and a `requires_state` term
/// must name a `party` datum (`DW0949`). A loop with no gate term holds forever
/// and is refused at the document (`DW0949`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Loop {
    /// Unique loop id (`loop/<kebab>`).
    pub id: LoopId,
    /// The slab a body crosses: an anchor-centred box (`anchor ± extent`), one
    /// axis of which is the crossing axis. The existing zone type, for the reason
    /// [`LethalVolume::region`] gives.
    pub region: StealthZone,
    /// Where the slab's own anchor cell lands: the loop's offset is
    /// `cell(to) − cell(region.anchor)`. Lies inside its anchor's piece
    /// (`DW0897`).
    pub to: Mark,
    /// Flags that must all be set for the loop to hold.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Flags any one of which stands the loop down — `forbids_flags:
    /// [flag/the-bell-found]` is a loop that ends the moment the bell is found.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Numeric gate terms: every comparison must hold for the loop to hold. Each
    /// names a `party`-scoped datum (`DW0949`). The third field of the one gate,
    /// carried by every gate consumer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_state: Vec<StateCompare>,
    /// A `party`-scoped datum the loop raises by one on every move, before
    /// `on_cross` runs — the counter a crossing-counted release reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts: Option<StateId>,
    /// The dungeon's answer to a move: effects run on every move, after the body
    /// is moved and the count raised, from the server command source (no acting
    /// player). Each effect's own `when` keys a write to a count. A `teleport`
    /// here is refused (`DW0949`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_cross: Vec<QuestEffect>,
}
