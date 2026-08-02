//! Deep quest/objective reachability analysis (ADR-0005 static layer, exit 2).
//!
//! This is the compiler's *semantic* reachability, distinct from the DSL's
//! *structural* stage-4 check (`DW0132`, convergent-sink). It merges the stage-5
//! trigger graph, intra-quest objective `after` ordering, and the stage-6
//! dialogue graph into one model and asks: can the finale actually complete —
//! and is the path it proves one a real player can walk?
//!
//! ## Model
//!
//! The reachability fixpoint itself lives in [`crate::flow`], which solves it
//! **per branch world** over a producer model that is conditional on its gating
//! context (flag gates, dialogue-branch XOR, reaction bundles). This module is
//! the diagnostic surface over that model:
//!
//! - A quest is **active** if triggered by `campaign-start`, or by
//!   `quest-complete(X)` where X can complete — in some world.
//! - An objective is **completable** if, in some world, its quest is active, all
//!   its `after` prerequisites are completable, its `requires_flags` are all
//!   producible, and its type is satisfiable (`talk-to`: a completing dialogue
//!   option is reachable from its NPC's tree `root` through options whose own
//!   gates are satisfied, and is that world's selected branch alternative).
//! - A quest **completes** if it is active and every objective is completable.
//! - The finale is reachable iff the finale quest completes in some world.
//! - The **exported critical path** is one world's playthrough, and it must
//!   replay legally step by step (`DW0204`).
//!
//! The lighting gate (`DW0210`/`DW0211`) is **no longer** here: spec-0010 moved it
//! to the compiler's assembled-world light model ([`crate::light`]), which measures
//! real light over the placed geometry (per-seam, sealed-cavity aware) instead of
//! reading per-piece admission profiles.
//!
//! ## Codes (`DW02xx`)
//!
//! | Code | Meaning |
//! |------|---------|
//! | `DW0201` | Finale quest can never complete (unreachable finale). |
//! | `DW0202` | Quest can never be triggered (unreachable / dead quest). |
//! | `DW0203` | Objective can never be completed (deadlock — e.g. an `after` chain that can't be satisfied). |
//! | `DW0204` | The exported critical path is not a coherent single-branch playthrough. |
//! | `DW0210`/`DW0211` | Assembled-light gate — see [`crate::light`]. |

use delvewright_dsl::{AnchorRegistry, Campaign, Diagnostic, Objective};

use crate::flow::Flow;

/// Stable analysis codes.
pub mod codes {
    /// Finale quest can never complete.
    pub const FINALE_UNREACHABLE: &str = "DW0201";
    /// Quest can never be triggered.
    pub const QUEST_UNREACHABLE: &str = "DW0202";
    /// Objective can never be completed (deadlock).
    pub const OBJECTIVE_DEADLOCK: &str = "DW0203";
    /// The exported critical path is not a walkable playthrough.
    pub const PATH_INCOHERENT: &str = crate::flow::DW_PATH_INCOHERENT;
    // DW0210 (dark-area mitigation) moved to `crate::light` (spec-0010): it is now
    // measured over the assembled world, not a per-piece admission profile.
}

/// Run reachability analysis. Returns `DW02xx` diagnostics (sorted); empty means
/// the finale and every quest/objective are reachable in at least one branch
/// world, and the exported critical path replays as a legal playthrough.
pub fn analyze_campaign(c: &Campaign, prefabs: &dyn AnchorRegistry) -> Vec<Diagnostic> {
    let quests = &c.quests.content.quests;
    let flow = Flow::new(c);
    let active = flow.any_active();
    let completable = flow.any_completable();
    let completed = flow.any_completed();

    let mut diags = Vec::new();

    // DW0202: quests never activated, in any world.
    for (i, q) in quests.iter().enumerate() {
        if !active.contains(q.id.as_str()) {
            diags.push(Diagnostic::error(
                codes::QUEST_UNREACHABLE,
                "quests",
                format!("/content/quests/{i}"),
                format!(
                    "quest `{}` can never be triggered — its `quest-complete` trigger names a \
                     quest that itself never completes (a dead branch). Point the trigger at a \
                     completable quest, or make the source quest completable",
                    q.id
                ),
            ));
        }
    }

    // DW0203: objectives in active quests that never become completable.
    for (qi, q) in quests.iter().enumerate() {
        if !active.contains(q.id.as_str()) {
            continue; // covered by DW0202
        }
        for (oi, obj) in q.objectives.iter().enumerate() {
            if !completable.contains(obj.id().as_str()) {
                let why = match obj {
                    Objective::TalkTo { npc, .. } => {
                        format!(
                            "no dialogue option reachable from `{npc}`'s root fires \
                             `complete-objective` for it in any branch of the campaign (or its \
                             prerequisites are unsatisfiable) — add a reachable completing \
                             option, or satisfy the blocking `after`/`requires_flags` \
                             prerequisite"
                        )
                    }
                    _ => "its `after` prerequisites or `requires_flags` can never all be \
                          satisfied in one branch of the campaign — break the unsatisfiable \
                          chain, or produce the gating flag from a producer reachable on the \
                          same branch (a `set-flag` gated on the very flag it sets, or one \
                          nested in an `on_caught`/`on_respawn` reaction bundle, is not a \
                          producer)"
                        .to_string(),
                };
                diags.push(Diagnostic::error(
                    codes::OBJECTIVE_DEADLOCK,
                    "quests",
                    format!("/content/quests/{qi}/objectives/{oi}"),
                    format!("objective `{}` can never be completed: {why}", obj.id()),
                ));
            }
        }
    }

    // DW0201: the finale never completes, in any world.
    let finale = c.quest_plan.content.finale.as_str();
    if !completed.contains(finale) {
        diags.push(Diagnostic::error(
            codes::FINALE_UNREACHABLE,
            "quest-plan",
            "/content/finale",
            format!(
                "finale quest `{finale}` can never complete (deep reachability) — some objective \
                 on the finale's completion path is unreachable. Look for the accompanying \
                 `DW0202`/`DW0203` on the blocking quest/objective and fix that; the finale \
                 clears once every quest on its `depends_on` chain can complete"
            ),
        ));
    } else {
        // DW0204: the path the compiler would export must replay as a legal
        // playthrough — every step activatable and completable at its position,
        // `campaign-complete` exactly at the end.
        let path = flow.playthrough();
        if let Err(f) = flow.replay(&path) {
            diags.push(Diagnostic::error(
                codes::PATH_INCOHERENT,
                "quests",
                "/content/quests".to_string(),
                f.message(),
            ));
        }
    }

    // DW0210 is no longer computed here (spec-0010): the lighting gate moved to the
    // compiler's assembled-world light model (`crate::light`), which measures real
    // light over the placed geometry rather than reading per-piece admission
    // profiles. `prefabs` is retained in the signature for the reachability model's
    // other uses and API stability.
    let _ = prefabs;

    diags.sort_by(|a, b| (&a.code, &a.path).cmp(&(&b.code, &b.path)));
    diags
}
