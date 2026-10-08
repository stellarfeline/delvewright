//! Ambushes: the resolved stage-5 `ambush` record and its collection.

use super::*;

/// A resolved stage-5 `ambush` (spec-0016 §3), collected in declared order —
/// the trigger cell and the cell each ambusher will stand on. An ambush whose
/// anchors do not resolve carries no entry (and so no proof); the desugared
/// trigger's own anchor checks report that.
#[derive(Clone, Debug)]
pub struct AmbushPlan {
    /// The full ambush id (`ambush/<kebab>`).
    pub id: String,
    /// The resolved trigger cell — where the player is standing when it springs.
    pub at: [i32; 3],
    /// One resolved spawn cell per ambusher, in declared order.
    pub actor_cells: Vec<[i32; 3]>,
}

/// Collect every stage-5 `ambush` (spec-0016 §3) in declared order, resolving the
/// trigger cell and each ambusher's spawn cell. An ambush whose trigger anchor or
/// whose every actor cell fails to resolve is skipped (the desugared trigger's own
/// anchor checks own that failure).
pub(super) fn collect_ambushes(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<AmbushPlan> {
    let by_id: BTreeMap<&str, &delvewright_dsl::Actor> = campaign
        .quests
        .content
        .actors
        .iter()
        .map(|a| (a.id.as_str(), a))
        .collect();
    let mut out = Vec::new();
    for amb in &campaign.quests.content.ambushes {
        let Some(at) = point_any(anchors, amb.at.as_str()) else {
            continue;
        };
        let actor_cells: Vec<[i32; 3]> = amb
            .actors
            .iter()
            .filter_map(|id| by_id.get(id.as_str()))
            .filter_map(|a| {
                point_any(anchors, a.anchor.as_str())
                    .map(|p| delvewright_dsl::offset_cell(p, a.offset))
            })
            .collect();
        out.push(AmbushPlan {
            id: amb.id.as_str().to_string(),
            at,
            actor_cells,
        });
    }
    out
}
