//! Quests: a quest's area and the quests a campaign starts with.

use super::*;

/// The area a stage-4 quest belongs to (free-function form of [`Plan::quest_area`],
/// usable before a [`Plan`] exists — e.g. from anchor collection).
pub(super) fn quest_area_of<'a>(campaign: &'a Campaign, quest_id: &str) -> Option<&'a str> {
    delvewright_dsl::quest_area(campaign, quest_id)
}

impl<'a> Plan<'a> {
    /// The area a stage-4 quest belongs to.
    pub fn quest_area(&self, quest_id: &str) -> Option<&str> {
        self.campaign
            .quest_plan
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == quest_id)
            .map(|q| q.area.as_str())
    }
}

/// Which quests are triggered by `campaign-start`.
pub fn campaign_start_quests(campaign: &Campaign) -> Vec<&str> {
    campaign
        .quests
        .content
        .quests
        .iter()
        .filter(|q| matches!(q.trigger, Trigger::CampaignStart))
        .map(|q| q.id.as_str())
        .collect()
}
