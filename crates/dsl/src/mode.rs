//! Modes: a state of the map in which every quest but the ones the mode
//! keeps is held (spec-0140).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ModeId, QuestId};

/// **A mode** (spec-0140): a state the story puts the map in. While it is
/// on, every quest the mode does not keep is **held**: none of its
/// objectives is told — no announcement, no journal page, no compass — none
/// completes, by its act or by any line, no press that serves one opens, and
/// its clocks stand still. A kept quest goes on as it always does.
///
/// The map is in one mode at a time, or in none. An `enter-mode` puts it in
/// the mode, leaving whichever was on. The mode is left by a `leave-mode`,
/// and by itself the moment none of its kept quests is in progress — started
/// and not complete — so a mode cannot outlast what it keeps. A class of
/// its own, over quests: the same declaration is a race the main story
/// waits for, a boss arena, a dream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Mode {
    /// Unique mode id (`mode/<kebab>`).
    pub id: ModeId,
    /// The quests that go on while the mode is on; every other quest is
    /// held. At least one (`DW1122`), each named once (`DW1122`).
    pub keeps: Vec<QuestId>,
}

impl Mode {
    /// Whether the mode keeps `quest`.
    pub fn keeps(&self, quest: &str) -> bool {
        self.keeps.iter().any(|q| q.as_str() == quest)
    }
}

impl crate::quest::QuestsContent {
    /// The mode `id`, when declared.
    pub fn mode(&self, id: &str) -> Option<&Mode> {
        self.modes.iter().find(|m| m.id.as_str() == id)
    }

    /// The number the map's mode score holds while mode `id` is on: its
    /// place in `modes`, from one. Zero is no mode.
    pub fn mode_number(&self, id: &str) -> Option<usize> {
        self.modes
            .iter()
            .position(|m| m.id.as_str() == id)
            .map(|i| i + 1)
    }

    /// The numbers of the modes that hold `quest` — every declared mode that
    /// does not keep it — ascending.
    pub fn modes_holding(&self, quest: &str) -> Vec<usize> {
        self.modes
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.keeps(quest))
            .map(|(i, _)| i + 1)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;

crate::dw_code! {
    /// (spec-0140 §6) **A mode that cannot be run as declared**: it keeps no
    /// quest (with nothing of its own in progress it is left the tick it is
    /// entered), or it keeps one quest twice. Validation-tier (exit 1).
    /// Prescription: name each quest the mode goes on with, once.
    pub const MODE_CANNOT_RUN: DwCode = DwCode::new("DW1122", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0140 §5) **A mode that holds what it waits on**: an objective of
    /// a quest the mode keeps, open while the mode is on, requires a flag
    /// that only bundles of quests the mode holds set — none of them a
    /// quest complete before the kept one starts. Once the mode is on
    /// nothing can set it, the kept quest never completes, and the mode is
    /// never left. Validation-tier (exit 1). Prescription: set the flag from
    /// a quest the mode keeps or from a trigger, keep the quest that sets
    /// it, or drop the gate.
    pub const MODE_HOLDS_ITS_KEY: DwCode = DwCode::new("DW1123", ExitTier::Build);
}

/// One `enter-mode` line: where it stands.
struct Entry {
    /// The quest and objective whose completion bundle fires it, when that
    /// is its root.
    objective: Option<(String, String)>,
}

/// Every objective of `quest` that is certainly settled when `objective`
/// completes: itself and its plain `after` ancestors.
fn settled_before<'a>(quest: &'a crate::Quest, objective: &'a str) -> BTreeSet<&'a str> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut stack = vec![objective];
    while let Some(cur) = stack.pop() {
        if !seen.insert(cur) {
            continue;
        }
        if let Some(o) = quest.objectives.iter().find(|o| o.id().as_str() == cur) {
            for a in o.after_required() {
                stack.push(a.as_str());
            }
        }
    }
    seen
}

/// Every quest certainly complete before `quest` starts: the chain of
/// `quest-complete` triggers above it. A flag one of them sets was set
/// before any objective of `quest` opened, whatever mode came after.
fn complete_before<'a>(c: &'a Campaign, quest: &'a str) -> BTreeSet<&'a str> {
    let quests = &c.quests.content.quests;
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut cur = quest;
    while let Some(crate::Trigger::QuestComplete { quest: above }) = quests
        .iter()
        .find(|q| q.id.as_str() == cur)
        .map(|q| &q.trigger)
    {
        if !seen.insert(above.as_str()) {
            break;
        }
        cur = above.as_str();
    }
    seen
}

/// **The objectives that may be open while mode `m` is on**, by `(quest
/// index, objective index)`: every objective of a quest it keeps, less the
/// ones certainly settled before every `enter-mode` of it fires — an entry
/// in a kept quest's own `on_objective_complete` bundle has that objective
/// and its `after` ancestors behind it. An entry anywhere else settles
/// nothing first.
fn open_under(c: &Campaign, m: &Mode, entries: &[Entry]) -> Vec<(usize, usize)> {
    let quests = &c.quests.content.quests;
    let mut out = Vec::new();
    for (qi, q) in quests.iter().enumerate() {
        if !m.keeps(q.id.as_str()) {
            continue;
        }
        // The objectives of `q` behind every entry.
        let mut behind: Option<BTreeSet<&str>> = None;
        for e in entries {
            let b = match &e.objective {
                Some((eq, eo)) if eq == q.id.as_str() => settled_before(q, eo),
                _ => BTreeSet::new(),
            };
            behind = Some(match behind {
                None => b,
                Some(prev) => prev.intersection(&b).copied().collect(),
            });
        }
        let behind = behind.unwrap_or_default();
        for (oi, o) in q.objectives.iter().enumerate() {
            if !behind.contains(o.id().as_str()) {
                out.push((qi, oi));
            }
        }
    }
    out
}

/// Stage-5 mode checks (spec-0140): id syntax and uniqueness, every
/// reference to a mode and a quest resolving (`DW0112`), the shape a mode can
/// be run in (`DW1122`), and no kept objective waiting on a flag only held
/// quests set (`DW1123`).
pub(crate) fn mode_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let q = &c.quests.content;
    let modes = &q.modes;
    for (i, m) in modes.iter().enumerate() {
        crate::ids::id_syntax!(d, m.id, "quests", format!("/content/modes/{i}/id"));
    }
    crate::ids::dup_check(
        modes
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id.as_str(), format!("/content/modes/{i}/id"))),
        "quests",
        "mode",
        d,
    );
    for (i, m) in modes.iter().enumerate() {
        if m.keeps.is_empty() {
            d.push(Diagnostic::error(
                MODE_CANNOT_RUN,
                "quests",
                format!("/content/modes/{i}/keeps"),
                format!(
                    "mode `{}` keeps no quest: a mode is on while a quest it keeps is in \
                     progress, so this one would be left the tick it is entered and hold \
                     nothing. Name in `keeps` each quest that goes on while the mode is on",
                    m.id
                ),
            ));
        }
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (k, kept) in m.keeps.iter().enumerate() {
            let path = format!("/content/modes/{i}/keeps/{k}");
            if !q.quests.iter().any(|x| x.id == *kept) {
                d.push(Diagnostic::error(
                    codes::DANGLING_REF,
                    "quests",
                    path,
                    format!(
                        "mode `{}` keeps unknown quest `{kept}` — name a quest this stage \
                         declares, or fix the reference",
                        m.id
                    ),
                ));
            } else if !seen.insert(kept.as_str()) {
                d.push(Diagnostic::error(
                    MODE_CANNOT_RUN,
                    "quests",
                    path,
                    format!(
                        "mode `{}` keeps quest `{kept}` twice: a quest is kept or held. Name \
                         it once",
                        m.id
                    ),
                ));
            }
        }
    }

    // Every `enter-mode` / `leave-mode` names a declared mode; each entry's
    // root is kept for `DW1123`.
    let mut entries: BTreeMap<&str, Vec<Entry>> = BTreeMap::new();
    crate::for_each_campaign_effect(c, &mut |path, site, e| {
        let (id, enters) = match &e.verb {
            crate::Verb::EnterMode { mode } => (mode, true),
            crate::Verb::LeaveMode { mode } => (mode, false),
            _ => return,
        };
        let Some(m) = q.mode(id.as_str()) else {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                path.to_string(),
                format!(
                    "`{}` names unknown mode `{id}` — declare it in the stage-5 `modes` list, \
                     or fix the reference",
                    e.verb.tag()
                ),
            ));
            return;
        };
        if enters {
            entries.entry(m.id.as_str()).or_default().push(Entry {
                objective: match site {
                    crate::EffectSite::Objective { quest, objective } => {
                        Some((quest.clone(), objective.clone()))
                    }
                    _ => None,
                },
            });
        }
    });

    // flag -> the quests whose bundles set it; and the flags something that
    // is no quest's bundle sets (a trigger, a trap, a shop, a dialogue
    // option, a disarm), which no mode holds.
    let mut by_quest: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut free: BTreeSet<&str> = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |_path, site, eff| {
        let crate::Verb::SetFlag { flag, .. } = &eff.verb else {
            return;
        };
        match site.quest() {
            Some(owner) => {
                if let Some(quest) = q.quests.iter().find(|x| x.id.as_str() == owner) {
                    by_quest
                        .entry(flag.as_str())
                        .or_default()
                        .insert(quest.id.as_str());
                }
            }
            None => {
                free.insert(flag.as_str());
            }
        }
    });
    for t in &c.dialogue.content.dialogues {
        for n in &t.nodes {
            for o in &n.options {
                for e in &o.effects {
                    if let crate::DialogueEffect::SetFlag { flag } = e {
                        free.insert(flag.as_str());
                    }
                }
            }
        }
    }
    for trap in &q.traps {
        if let Some(dis) = &trap.disarm {
            free.insert(dis.sets_flag.as_str());
        }
    }
    for g in &q.timed_gates {
        if let Some(dis) = &g.disarm {
            free.insert(dis.sets_flag.as_str());
        }
    }

    for m in modes {
        let Some(es) = entries.get(m.id.as_str()) else {
            continue;
        };
        for (qi, oi) in open_under(c, m, es) {
            let quest = &q.quests[qi];
            let o = &quest.objectives[oi];
            for (fi, f) in o.requires_flags().iter().enumerate() {
                let flag = f.as_str();
                if free.contains(flag) {
                    continue;
                }
                let Some(setters) = by_quest.get(flag) else {
                    continue; // never set at all: `DW0172`'s finding
                };
                // Set by a quest the mode keeps, or by one complete before
                // this quest starts.
                let before = complete_before(c, quest.id.as_str());
                if setters.iter().any(|s| m.keeps(s) || before.contains(s)) {
                    continue;
                }
                let names = setters.iter().copied().collect::<Vec<_>>().join("`, `");
                d.push(Diagnostic::error(
                    MODE_HOLDS_ITS_KEY,
                    "quests",
                    format!("/content/quests/{qi}/objectives/{oi}/requires_flags/{fi}"),
                    format!(
                        "mode `{}` keeps quest `{}`, whose objective `{}` requires flag \
                         `{flag}`, and the only effects that set `{flag}` are in bundles of \
                         `{names}`, which the mode holds: once the mode is on nothing can set \
                         it, the objective never opens, and the party is left in a mode with \
                         nothing to do. Set `{flag}` from a quest the mode keeps or from a \
                         trigger, add the quest that sets it to the mode's `keeps`, or drop \
                         the gate",
                        m.id,
                        quest.id,
                        o.id()
                    ),
                ));
            }
        }
    }
}
