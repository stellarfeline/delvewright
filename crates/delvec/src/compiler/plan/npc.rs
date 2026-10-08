//! NPCs: the planned NPC and its dialogue options.

use super::*;

/// A dialogue plan for one NPC.
pub struct NpcPlan {
    /// NPC id.
    pub npc_id: String,
    /// Sanitized local name (`keeper`).
    pub safe: String,
    /// The trigger objective (`dw.dlg_<npc>`).
    pub trigger_objective: String,
    /// Entity tag on the interaction entity (`dw_npc_<npc>`).
    pub tag: String,
    /// Root dialogue node id.
    pub root: String,
    /// Options in a stable order, each with its trigger value.
    pub options: Vec<OptionPlan>,
}

/// One dialogue option and the `/trigger` value it fires.
pub struct OptionPlan {
    /// Trigger value (`/trigger dw.dlg_<npc> set <n>`), 1-based across the NPC.
    pub n: i32,
    /// The node this option belongs to.
    pub node_id: String,
    /// Button label.
    pub label: String,
    /// The button's hover tooltip (DSL v0.8) — the full line the label captions.
    /// `None` emits no `tooltip` key at all, so a campaign that authors none is
    /// byte-identical to a pre-0.8 build.
    pub tooltip: Option<String>,
    /// Navigation target node, if any.
    pub next: Option<String>,
    /// Objectives this option completes.
    pub completes: Vec<String>,
    /// Flags this option sets when chosen (DSL v0.4 dialogue `set-flag`).
    pub sets_flags: Vec<String>,
    /// Flags that must be set for this option to be shown (DSL v0.4).
    pub requires_flags: Vec<String>,
    /// Flags whose being set HIDES this option (DSL v0.6 negative gate).
    pub forbids_flags: Vec<String>,
    /// Numeric gate terms (DSL v0.10, spec-0031): the option is shown only while
    /// every comparison holds.
    pub requires_state: Vec<delvewright_dsl::StateCompare>,
    /// World-time cuts this option fires (DSL v0.5 dialogue `set-time`), in order.
    pub sets_time: Vec<delvewright_dsl::WorldTime>,
    /// Weather cuts this option fires (DSL v0.5 dialogue `set-weather`), in order.
    pub sets_weather: Vec<delvewright_dsl::WorldWeather>,
    /// Checkpoints this option sets (DSL v0.6 dialogue `set-checkpoint`), each
    /// `(anchor, on_respawn)`, in order.
    pub sets_checkpoints: Vec<(String, Vec<QuestEffect>)>,
    /// Deferred NPCs this option summons (DSL v0.6 dialogue `spawn-npc`), in order.
    pub spawns_npcs: Vec<String>,
}

impl<'a> Plan<'a> {
    /// The area an NPC or quest belongs to.
    pub fn npc_area(&self, npc_id: &str) -> Option<&str> {
        self.campaign
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc_id)
            .map(|n| n.area.as_str())
    }
}

pub(super) fn plan_npc(npc: &Npc, tree: &NpcDialogue) -> NpcPlan {
    let safe = safe_local(npc.id.as_str());
    let mut options = Vec::new();
    let mut n = 0;
    for node in &tree.nodes {
        for opt in &node.options {
            n += 1;
            let mut completes = Vec::new();
            let mut sets_flags = Vec::new();
            let mut sets_time = Vec::new();
            let mut sets_weather = Vec::new();
            let mut sets_checkpoints = Vec::new();
            let mut spawns_npcs = Vec::new();
            for e in &opt.effects {
                match e {
                    DialogueEffect::CompleteObjective { objective } => {
                        completes.push(objective.as_str().to_string());
                    }
                    DialogueEffect::SetFlag { flag } => {
                        sets_flags.push(flag.as_str().to_string());
                    }
                    DialogueEffect::SetTime { time } => sets_time.push(*time),
                    DialogueEffect::SetWeather { weather } => sets_weather.push(*weather),
                    DialogueEffect::SetCheckpoint { anchor, on_respawn } => {
                        sets_checkpoints.push((anchor.as_str().to_string(), on_respawn.clone()));
                    }
                    DialogueEffect::SpawnNpc { npc } => {
                        spawns_npcs.push(npc.as_str().to_string());
                    }
                }
            }
            options.push(OptionPlan {
                n,
                node_id: node.id.as_str().to_string(),
                label: opt.label.clone(),
                tooltip: opt.tooltip.clone(),
                next: opt
                    .next
                    .as_ref()
                    .map(|d: &DialogueId| d.as_str().to_string()),
                completes,
                sets_flags,
                requires_flags: opt
                    .requires_flags
                    .iter()
                    .map(|f| f.as_str().to_string())
                    .collect(),
                forbids_flags: opt
                    .forbids_flags
                    .iter()
                    .map(|f| f.as_str().to_string())
                    .collect(),
                requires_state: opt.requires_state.clone(),
                sets_time,
                sets_weather,
                sets_checkpoints,
                spawns_npcs,
            });
        }
    }
    NpcPlan {
        npc_id: npc.id.as_str().to_string(),
        trigger_objective: dlg_trigger(npc.id.as_str()),
        tag: format!("dw_npc_{safe}"),
        root: tree.root.as_str().to_string(),
        safe,
        options,
    }
}
