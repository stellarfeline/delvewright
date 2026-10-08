//! Objectives: the quest an objective belongs to and the effects it fires.

use super::*;

/// The stage-5 quest containing an objective id, and that objective.
pub fn objective_quest<'a>(
    campaign: &'a Campaign,
    obj_id: &str,
) -> Option<(&'a str, &'a Objective)> {
    for q in &campaign.quests.content.quests {
        for o in &q.objectives {
            if o.id().as_str() == obj_id {
                return Some((q.id.as_str(), o));
            }
        }
    }
    None
}

/// The `on_objective_complete` effects for an objective, across the campaign.
pub fn objective_effects<'a>(campaign: &'a Campaign, obj_id: &str) -> Vec<&'a QuestEffect> {
    let mut out = Vec::new();
    for q in &campaign.quests.content.quests {
        if let Some(effects) = q
            .on_objective_complete
            .get(&delvewright_dsl::ObjectiveId(obj_id.to_string()))
        {
            out.extend(effects.iter());
        }
    }
    out
}
