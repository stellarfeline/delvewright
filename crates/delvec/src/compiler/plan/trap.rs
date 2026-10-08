//! Traps: the resolved trap record and its disarm.

use super::*;

/// A resolved trap (DSL v0.6, spec-0011), collected in deterministic content
/// order. Carries everything the nav proof (`DW0342`), the payload/disarm
/// emission, and the PackTest need.
#[derive(Clone, Debug)]
pub struct TrapPlan {
    /// The raw trap id (`trap/<name>`).
    pub id: String,
    /// Sanitized local name (`dart_hall`) for emitted function/tag names.
    pub safe: String,
    /// The declared trigger kind (informs the hazard model + PackTest).
    pub trigger: TrapTrigger,
    /// The `anchor/trap` marker this trap sits on — the key into prefab metadata
    /// for its hardware declarations (`dispenser`, `trigger_block`).
    pub at_anchor: String,
    /// The resolved absolute trigger/hazard cell (the trap's `at` anchor cell).
    pub trigger_cell: [i32; 3],
    /// The resolved absolute dispenser socket cell (from the `at` anchor's
    /// `dispenser` metadata), or `None` if the prefab exposes none.
    pub dispenser: Option<[i32; 3]>,
    /// The dispense payload `(item, count)` this trap loads, if any.
    pub payload: Option<(String, u32)>,
    /// The spec-0022 **command payload**: the ordered effect bundle the trigger
    /// fires. Empty for a pure spec-0011 redstone trap, which is what keeps such
    /// a campaign's output byte-identical.
    pub payload_effects: Vec<QuestEffect>,
    /// How dangerous the trap is.
    pub lethality: Lethality,
    /// Whether the trap re-arms after firing.
    pub reset: TrapReset,
    /// The resolved disarm affordance, if declared.
    pub disarm: Option<TrapDisarmPlan>,
    /// Flags that gate the trap being active.
    pub requires_flags: Vec<String>,
    /// Flags whose being set deactivates the trap (DSL v0.6 negative gate).
    pub forbids_flags: Vec<String>,
    /// Numeric gate terms (DSL v0.10, spec-0031): the trap is armed only while
    /// every comparison holds.
    pub requires_state: Vec<delvewright_dsl::StateCompare>,
}

/// A resolved trap disarm affordance (DSL v0.6, spec-0011).
#[derive(Clone, Debug)]
pub struct TrapDisarmPlan {
    /// The disarm anchor name (`anchor/…`).
    pub via_anchor: String,
    /// The resolved absolute cell of the disarm affordance.
    pub via_cell: [i32; 3],
    /// The flag the disarm sets.
    pub sets_flag: String,
}

pub(super) fn collect_traps(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    dispenser_cells: &BTreeMap<(String, String), [i32; 3]>,
) -> Vec<TrapPlan> {
    let mut out = Vec::new();
    for t in &campaign.quests.content.traps {
        let Some(trigger_cell) = point_any(anchors, t.at.as_str()) else {
            continue;
        };
        let dispenser = dispenser_cells
            .iter()
            .find(|((_, name), _)| name == t.at.as_str())
            .map(|(_, cell)| *cell);
        let payload = t.dispense().map(|(item, count)| (item.to_string(), count));
        let payload_effects = t.payload.clone();
        let disarm = t.disarm.as_ref().and_then(|dis| {
            point_any(anchors, dis.via.as_str()).map(|via_cell| TrapDisarmPlan {
                via_anchor: dis.via.as_str().to_string(),
                via_cell,
                sets_flag: dis.sets_flag.as_str().to_string(),
            })
        });
        out.push(TrapPlan {
            id: t.id.as_str().to_string(),
            safe: safe_local(t.id.as_str()),
            trigger: t.trigger,
            at_anchor: t.at.as_str().to_string(),
            trigger_cell,
            dispenser,
            payload,
            payload_effects,
            lethality: t.lethality,
            reset: t.reset,
            disarm,
            requires_flags: t
                .requires_flags
                .iter()
                .map(|f| f.as_str().to_string())
                .collect(),
            requires_state: t.requires_state.clone(),
            forbids_flags: t
                .forbids_flags
                .iter()
                .map(|f| f.as_str().to_string())
                .collect(),
        });
    }
    out
}
