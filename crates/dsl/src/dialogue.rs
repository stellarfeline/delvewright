//! Stage 6 — dialogue: each NPC's branching conversation, its options and effects.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    AnchorId, DialogueId, FlagId, Happening, NpcId, ObjectiveId, QuestEffect, StateCompare,
    WorldTime, WorldWeather,
};

#[cfg(doc)]
use crate::Verb;

/// Stage 6 payload: one dialogue tree per stage-2 NPC (spec-0001 v0.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DialogueContent {
    /// One tree per NPC (1:1 with stage 2, both directions).
    pub dialogues: Vec<NpcDialogue>,
}

impl DialogueContent {
    /// The dialogue tree for an NPC id, if present.
    pub fn tree_for(&self, npc: &str) -> Option<&NpcDialogue> {
        self.dialogues.iter().find(|t| t.npc.as_str() == npc)
    }
}

/// One NPC's dialogue tree: a root node plus a set of nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcDialogue {
    /// The NPC this tree belongs to (stage-2 ref).
    pub npc: NpcId,
    /// The entry node id; every node must be reachable from it.
    pub root: DialogueId,
    /// The dialogue nodes.
    pub nodes: Vec<DialogueNode>,
}

impl NpcDialogue {
    /// The ids of the nodes reachable from `roots` by following option `next`
    /// edges, ignoring every option gate.
    ///
    /// **The one authority for "what can this tree show, entered here".** A
    /// dialogue tree has more than one entry point — the stage-6 `root`, and
    /// every node a quest's `cast` ledger names as a scene — so "reachable" is
    /// always relative to a root SET, and each consumer supplies the set its own
    /// question is about. `DW0120`/`DW0123` ask about every entry point at once;
    /// the cast ledger's `DW0858` asks about the scenes live during one
    /// objective. Gates are ignored on purpose: an option's flag gate is
    /// `DW0191`'s question, not this one's.
    ///
    /// Root ids that name no node of this tree contribute nothing (a dangling
    /// scene root is `DW0464`, and a dangling stage-6 `root` is `DW0121`).
    pub fn reachable_from<'a>(&'a self, roots: &[&str]) -> BTreeSet<&'a str> {
        let by_id: BTreeMap<&'a str, &'a DialogueNode> =
            self.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let mut seen: BTreeSet<&'a str> = BTreeSet::new();
        let mut stack: Vec<&'a str> = roots
            .iter()
            .filter_map(|r| by_id.get_key_value(*r).map(|(k, _)| *k))
            .collect();
        while let Some(cur) = stack.pop() {
            if !seen.insert(cur) {
                continue;
            }
            let Some(node) = by_id.get(cur) else { continue };
            for opt in &node.options {
                if let Some(next) = &opt.next
                    && let Some((k, _)) = by_id.get_key_value(next.as_str())
                {
                    stack.push(k);
                }
            }
        }
        seen
    }

    /// The objective ids some option reachable from `roots` completes — what a
    /// player entering this tree at those scenes can actually finish.
    pub fn completes_from<'a>(&'a self, roots: &[&str]) -> BTreeSet<&'a str> {
        let seen = self.reachable_from(roots);
        let mut out: BTreeSet<&'a str> = BTreeSet::new();
        for node in &self.nodes {
            if !seen.contains(node.id.as_str()) {
                continue;
            }
            for opt in &node.options {
                for eff in &opt.effects {
                    if let DialogueEffect::CompleteObjective { objective } = eff {
                        out.insert(objective.as_str());
                    }
                }
            }
        }
        out
    }
}

/// One dialogue node: text plus branching options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DialogueNode {
    /// Node id (unique within this NPC's dialogue).
    pub id: DialogueId,
    /// The line the NPC speaks.
    pub text: String,
    /// Branching options; empty closes the dialog.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<DialogueOption>,
}

/// One selectable dialogue option.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DialogueOption {
    /// Button label.
    pub label: String,
    /// The full line this button is the caption of (DSL v0.8). Vanilla's dialog button codec
    /// (`CommonButtonData`) carries an optional `tooltip` component beside
    /// `label`, and the client hangs it on the button as a real hover tooltip —
    /// so a caption on the button and the sentence the character actually says
    /// can both exist. **Not** subject to `DW0331`: a tooltip is not drawn on the
    /// 150 px button, it is wrapped at 170 px into its own hover box, so it never
    /// scrolls. Player-visible, so it translates like the label
    /// (`dlg.<npc>.<node>.opt.<i>.tooltip`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    /// Next node; omitted closes the dialog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<DialogueId>,
    /// Flags that must be set for this option to be shown (DSL v0.4). Mirrors an
    /// objective's `requires_flags`: an ungated option (empty) is unchanged; a
    /// gated option is hidden until every referenced flag has been set by a
    /// `set-flag` effect (quest or dialogue). Validation guarantees a flag-gated
    /// option cannot make a critical-path node unreachable (`DW0191`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Negative flag gate (DSL v0.6): the option is
    /// **hidden** (and its `/trigger` handler inert) while ANY listed flag is set
    /// for the player — the dual of `requires_flags`. A `forbids_flags`-gated
    /// option counts as *gated* for the `DW0191` deadlock guard: it can be
    /// suppressed at any point, so it cannot be the only completing path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Numeric gate terms (DSL v0.10, spec-0031): every listed comparison must
    /// hold for this gate to be open. The third field of the one gate, carried by
    /// every gate consumer — never by the verb that first wanted it. Default
    /// empty, so a pre-0.10 campaign is byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_state: Vec<StateCompare>,
    /// Effects fired when this option is chosen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<DialogueEffect>,
    /// What choosing this option does to the story (spec-0025). Required for a
    /// **story-weight** beat
    /// — an option carrying a `set-flag` effect, which is how a player's choice
    /// forks the world (`DW0481`). An option that only walks the tree or
    /// completes an objective needs none: the objective already declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub happening: Option<Happening>,
}

/// Effect fired by a dialogue option. `complete-objective` (v0.2) and, from DSL
/// v0.4, `set-flag` (mirrors the quest effect — sets a campaign flag from a
/// dialogue choice, enabling flag-gated options/objectives/triggers).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DialogueEffect {
    /// Marks a stage-5 objective complete.
    CompleteObjective {
        /// The objective to complete (resolved at the stage-5 boundary).
        objective: ObjectiveId,
    },
    /// Sets a campaign flag (DSL v0.4), mirroring [`Verb::SetFlag`].
    SetFlag {
        /// The flag to set.
        flag: FlagId,
    },
    /// Cuts the world time (DSL v0.5), mirroring [`Verb::SetTime`].
    SetTime {
        /// The time state to cut to.
        time: WorldTime,
    },
    /// Cuts the weather (DSL v0.5), mirroring [`Verb::SetWeather`].
    SetWeather {
        /// The weather state to cut to.
        weather: WorldWeather,
    },
    /// Sets the party-wide respawn checkpoint (DSL v0.6, spec-0012), mirroring
    /// [`Verb::SetCheckpoint`] — usable from a dialogue outcome.
    SetCheckpoint {
        /// The prefab checkpoint anchor the party respawns at.
        anchor: AnchorId,
        /// Per-player effects re-run on respawn while this checkpoint is active
        /// (scene reset). Empty = no hook.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_respawn: Vec<QuestEffect>,
    },
    /// Summons a `deferred` stage-2 NPC (DSL v0.6), mirroring
    /// [`Verb::SpawnNpc`] — a character who walks in mid-conversation.
    SpawnNpc {
        /// The NPC (stage-2 ref) to summon.
        npc: NpcId,
    },
}

impl DialogueEffect {
    /// The v0.5 effect name if this dialogue effect is one introduced in DSL v0.5
    /// (`set-time`/`set-weather`, spec-0010).
    pub fn v05_effect(&self) -> Option<&'static str> {
        match self {
            DialogueEffect::SetTime { .. } => Some("set-time"),
            DialogueEffect::SetWeather { .. } => Some("set-weather"),
            _ => None,
        }
    }

    /// The v0.6 effect name if this dialogue effect is one introduced in DSL v0.6
    /// (`set-checkpoint`, spec-0012; `spawn-npc`).
    pub fn v06_effect(&self) -> Option<&'static str> {
        match self {
            DialogueEffect::SetCheckpoint { .. } => Some("set-checkpoint"),
            DialogueEffect::SpawnNpc { .. } => Some("spawn-npc"),
            _ => None,
        }
    }

    /// The NPC id if this is a v0.6 `spawn-npc` dialogue effect.
    pub fn spawn_npc(&self) -> Option<&NpcId> {
        match self {
            DialogueEffect::SpawnNpc { npc } => Some(npc),
            _ => None,
        }
    }

    /// **The body this dialogue effect puts into the world**, by id — the
    /// dialogue half of [`QuestEffect::body_entry`].
    ///
    /// A body can enter the world from a conversation as well as from a quest
    /// bundle, and a rule about what is standing where has to enumerate **every**
    /// entry point or it is a gate with a door beside it. This enum carries no
    /// exit at all: nothing a dialogue option does removes a body.
    pub fn body_entry(&self) -> Option<&str> {
        match self {
            DialogueEffect::SpawnNpc { npc } => Some(npc.as_str()),
            _ => None,
        }
    }

    /// `(anchor, on_respawn)` if this is a v0.6 `set-checkpoint` dialogue effect.
    pub fn set_checkpoint(&self) -> Option<(&AnchorId, &[QuestEffect])> {
        match self {
            DialogueEffect::SetCheckpoint { anchor, on_respawn } => {
                Some((anchor, on_respawn.as_slice()))
            }
            _ => None,
        }
    }

    /// The `on_respawn` bundle of a v0.6 `set-checkpoint` dialogue effect —
    /// **effect root 5** ([`crate::effects`]). Named separately from
    /// [`Self::set_checkpoint`] so the root walk and its mutable mirror name the
    /// same accessor modulo mutability, which is what lets one macro body generate
    /// both.
    pub fn set_checkpoint_on_respawn(&self) -> Option<&[QuestEffect]> {
        match self {
            DialogueEffect::SetCheckpoint { on_respawn, .. } => Some(on_respawn.as_slice()),
            _ => None,
        }
    }

    /// The `on_respawn` bundle of a v0.6 `set-checkpoint` dialogue effect, exposed
    /// mutably so the localization pass can rewrite the player-visible strings
    /// nested inside it. Lockstep sibling of [`Self::set_checkpoint`] — the bundle
    /// is a plain `Vec<QuestEffect>` that emission really lowers, so every scan
    /// that reaches it read-only needs a way to reach it writable too.
    pub fn set_checkpoint_on_respawn_mut(&mut self) -> Option<&mut [QuestEffect]> {
        match self {
            DialogueEffect::SetCheckpoint { on_respawn, .. } => Some(on_respawn.as_mut_slice()),
            _ => None,
        }
    }
}

impl DialogueEffect {
    /// The `set-flag` flag id if this is a v0.4 `set-flag` dialogue effect.
    pub fn set_flag(&self) -> Option<&FlagId> {
        match self {
            DialogueEffect::SetFlag { flag } => Some(flag),
            _ => None,
        }
    }

    /// The v0.4 effect name if this dialogue effect is one introduced in DSL v0.4
    /// (`set-flag`).
    pub fn v04_effect(&self) -> Option<&'static str> {
        match self {
            DialogueEffect::SetFlag { .. } => Some("set-flag"),
            _ => None,
        }
    }

    /// The target time if this is a v0.5 `set-time` dialogue effect.
    pub fn set_time(&self) -> Option<WorldTime> {
        match self {
            DialogueEffect::SetTime { time } => Some(*time),
            _ => None,
        }
    }

    /// The target weather if this is a v0.5 `set-weather` dialogue effect.
    pub fn set_weather(&self) -> Option<WorldWeather> {
        match self {
            DialogueEffect::SetWeather { weather } => Some(*weather),
            _ => None,
        }
    }
}
