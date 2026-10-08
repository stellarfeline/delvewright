//! Shortcuts: the resolved stage-5 `shortcut` record and its collection.

use super::*;

/// A resolved stage-5 `shortcut` (spec-0016 §2), collected in deterministic
/// content order. A shortcut whose `gate` is not a resolvable gate region, or
/// whose `unlock` anchor does not resolve to a point, carries no plan entry (and
/// so no emission and no proof) — `DW0371` rejects those at validation.
#[derive(Clone, Debug)]
pub struct ShortcutPlan {
    /// The full shortcut id (`shortcut/<kebab>`).
    pub id: String,
    /// The function/tag-safe local id.
    pub safe: String,
    /// The gate anchor name.
    pub gate_anchor: String,
    /// The gate region's inclusive corners (absolute world coords).
    pub gate_region: ([i32; 3], [i32; 3]),
    /// The block the gate region is filled with (cleared to air on unlock).
    pub gate_block: String,
    /// The unlock anchor name.
    pub unlock_anchor: String,
    /// The resolved far-side unlock cell.
    pub unlock: [i32; 3],
    /// Effects fired once, when the shortcut opens.
    pub on_unlock: Vec<QuestEffect>,
    /// The volume a presser must stand in for the press to count as coming from
    /// the wrong side, derived from the gate slab and the `unlock` cell. `None`
    /// when the geometry does not decide it — `DW0425`.
    pub sealed_side: Option<crate::compiler::wrongside::SealedSide>,
}

impl ShortcutPlan {
    /// The **shell** cells of the sealed gate: every region cell with at least
    /// one axis-neighbour outside the region, in ascending `(x, y, z)` order —
    /// exactly the clickable surface, and for the thin slab a doorway usually is,
    /// the whole region.
    ///
    /// The same rule [`SealHintPlan::shell_cells`] applies to a `close-gate`
    /// seal, for the same reason: a cell buried inside the door has six sealed
    /// neighbours, so no face of it can ever be in a crosshair, and arming it
    /// would ship an entity nothing can reach.
    pub fn shell_cells(&self) -> Vec<[i32; 3]> {
        shell_cells_of(self.gate_region)
    }
}

/// Collect every stage-5 `shortcut` (spec-0016 §2) in declared order, resolving
/// its gate region and far-side unlock cell. A shortcut whose anchors do not
/// resolve is skipped here (validation owns that, `DW0371`).
pub(super) fn collect_shortcuts(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<ShortcutPlan> {
    let mut out = Vec::new();
    for sc in &campaign.quests.content.shortcuts {
        let Some((from, to, block)) = gate_region_block_any(anchors, sc.gate.as_str()) else {
            continue;
        };
        let Some(unlock) = point_any(anchors, sc.unlock.as_str()) else {
            continue;
        };
        out.push(ShortcutPlan {
            id: sc.id.as_str().to_string(),
            safe: safe_local(sc.id.as_str()),
            gate_anchor: sc.gate.as_str().to_string(),
            gate_region: (from, to),
            gate_block: block,
            unlock_anchor: sc.unlock.as_str().to_string(),
            unlock,
            on_unlock: sc.on_unlock.clone(),
            // Which half of the doorway is the sealed one, from the
            // slab's thin axis and the side the unlock stands on. `None` is not
            // an error here — `emit` raises `DW0425` only if an answer was
            // actually authored for a side the geometry does not name.
            sealed_side: crate::compiler::wrongside::derive((from, to), unlock),
        });
    }
    out
}
