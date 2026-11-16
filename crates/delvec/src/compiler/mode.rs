//! **A mode over the path** (spec-0140 §5) — `DW1124`, and the binding line.
//!
//! A mode is a state of the map in which every quest but the ones it keeps
//! is held; the surface is `delvewright_dsl::mode` and the emission
//! `emit::mode`. That a mode is always left is three facts. The emitted
//! settlement leaves a mode the moment none of its kept quests is in
//! progress, so no mode outlasts what it keeps. The document tier refuses a
//! kept objective that waits on a flag only held quests set (`DW1123`), so
//! what a mode keeps is not locked by what it holds. And here the map's mode
//! is replayed along every path the build proves
//! ([`crate::compiler::plan::walk_modes`]): a step that performs an
//! objective of a quest the mode then on does not keep is `DW1124`.

use delvewright_dsl::{DwCode, ExitTier, Verb};

use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW1124`: a path the build proves performs an objective while the map
    /// is in a mode that does not keep its quest (spec-0140 §5). The
    /// objective is held there — not told, not completable — so the path
    /// stops at that step. The message names the objective, its quest, the
    /// step, the mode and the step it is entered at.
    pub const DW_PATH_WALKS_HELD: DwCode = DwCode::new("DW1124", ExitTier::Build);
}

/// What the mode proof read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModeGate {
    /// Modes declared.
    pub modes: usize,
    /// Of those, the ones some `enter-mode` names.
    pub entered: usize,
    /// Quests kept, summed over the modes.
    pub kept: usize,
    /// Quests held, summed over the modes.
    pub held: usize,
    /// Clocks stopped at an entry and started again at the leaving, summed
    /// over the modes.
    pub clocks: usize,
    /// Spans a mode is on over the exported path.
    pub spans: usize,
    /// Of those, the ones the path leaves before it ends.
    pub left: usize,
    /// Objective steps the exported path performs while a mode is on.
    pub judged: usize,
}

impl ModeGate {
    /// The one line this proof owes its reader, on every build that declares
    /// a mode.
    pub fn line(&self) -> String {
        format!(
            "mode binding (DW1122–DW1124): {} mode(s), {} entered by a line; {} quest(s) kept \
             and {} held over them, {} clock(s) stood still; on the exported path {} span(s) \
             of a mode, {} left before its end, {} objective step(s) inside judged for a quest \
             the mode keeps",
            self.modes,
            self.entered,
            self.kept,
            self.held,
            self.clocks,
            self.spans,
            self.left,
            self.judged
        )
    }
}

/// Whether the campaign declares a mode.
pub fn declares_one(plan: &Plan) -> bool {
    !plan.campaign.quests.content.modes.is_empty()
}

/// The binding of a plan that built: its path was walked without a refusal.
pub fn gate(plan: &Plan) -> ModeGate {
    let c = plan.campaign;
    let q = &c.quests.content;
    let mut entered: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    crate::compiler::plan::for_each_gate_effect(c, &mut |_, e| {
        if let Verb::EnterMode { mode } = &e.verb {
            entered.insert(mode.as_str());
        }
    });
    let walk = crate::compiler::plan::walk_modes(c, &plan.path_firing).unwrap_or_default();
    ModeGate {
        modes: q.modes.len(),
        entered: q
            .modes
            .iter()
            .filter(|m| entered.contains(m.id.as_str()))
            .count(),
        kept: q.modes.iter().map(|m| m.keeps.len()).sum(),
        held: q
            .modes
            .iter()
            .map(|m| q.quests.iter().filter(|x| !m.keeps(x.id.as_str())).count())
            .sum(),
        clocks: q
            .modes
            .iter()
            .map(|m| crate::compiler::emit::clocks_held_by(c, m).len())
            .sum(),
        spans: walk.spans.len(),
        left: walk.spans.iter().filter(|s| s.2.is_some()).count(),
        judged: walk.judged,
    }
}
