//! Ambushes: a fight sprung on the party at a place.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    ActorId, AmbushId, AnchorId, EnvTrigger, Happening, QuestEffect, TriggerAudience, TriggerId,
    TriggerOn, Verb,
};

#[cfg(doc)]
use crate::QuestsContent;

/// A stage-5 **ambush** (spec-0016 §3) — one declaration for a beat that
/// otherwise takes a deferred actor set plus a hand-wired trigger.
///
/// **`telegraph` is optional, and that is a design ruling, not an oversight.**
/// The un-telegraphed ambush — the shove off the cliff you
/// could not have known about — is core souls vocabulary: 初见杀 is how the level
/// teaches. The engine does not sand that edge off.
///
/// What the engine *does* owe the player is **counterplay on the retry**: having
/// died once, an informed player must have something to do about it. Determinism
/// guarantees the second attempt meets the same ambushers in the same cells; the
/// compiler adds the missing half — `DW0376` proves the trigger cell is not a
/// sealed pocket, i.e. that with every ambusher standing where it will stand,
/// a route out still exists (a retreat, luring ground, a positioning line).
/// Dying uninformed is a lesson; dying with no play available is a broken beat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ambush {
    /// Unique ambush id (`ambush/<kebab>`).
    pub id: AmbushId,
    /// The anchor the trigger watches — the corner, doorway or ledge the player
    /// walks into.
    pub at: AnchorId,
    /// The stage-5 actors that spring (1..N). Each is summoned at its own
    /// declared anchor and immediately unleashed to real AI.
    pub actors: Vec<ActorId>,
    /// What springs it (`approach{range}` / `strike` / `use`), exactly the v0.4
    /// environment-trigger vocabulary.
    pub trigger: TriggerOn,
    /// The **optional** tell, fired at the trigger before the ambushers exist:
    /// a sound, a shadow, a line of narration. Empty = un-telegraphed, which is
    /// fully legal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telegraph: Vec<QuestEffect>,
    /// What the spring does to the story (DSL v0.8, spec-0025; required at
    /// 0.8.0, `DW0481`).
    ///
    /// **The declaration lives here because this is the object the author
    /// wrote.** An ambush desugars into a `spawn-actor` plus an `unleash-actor`
    /// per listed actor, and every one of those is a story node `DW0481` demands
    /// a `happening` from — on effects the author never wrote and cannot reach.
    /// So for as long as this field did not exist, an `ambushes[]` entry could
    /// not compile at 0.8.0 or above at all: declared in the schema, accepted by
    /// the schema check, and refused at validation with a prescription naming a
    /// field that was not there. Nothing caught it because no campaign and no
    /// fixture had ever written an ambush at a version where the obligation was
    /// live — which is the case spec-0039 exists to make impossible.
    ///
    /// An ambush is **one** beat, not `2N` of them: the ambushers appearing and
    /// coming at you is a single dramatic moment, so one declaration covers the
    /// whole spring. [`Self::to_trigger`] stamps it onto the first generated
    /// `spawn-actor` so the chronicle carries the line at the right position,
    /// and `branch::check_happenings` reads the obligation off the ambush rather
    /// than off the beats derived from it. Repeating it on all `2N` is the
    /// obvious alternative and it is wrong twice over: the chronicle would gain
    /// `N` duplicate lines, and `DW0485` would see one subject act repeatedly
    /// and be right to call it a contradiction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub happening: Option<Happening>,
}

impl Ambush {
    /// The environment trigger this ambush desugars to (spec-0016 §3): a
    /// one-shot trigger at `at` whose effects are the telegraph, then a
    /// `spawn-actor` + `unleash-actor` per listed actor, in declared order.
    ///
    /// This is the **single** expansion authority. Every consumer — validation,
    /// the l10n inventory, the flag/wave producer scans, nav, emission — reads
    /// triggers through [`QuestsContent::all_triggers`], so the sugar cannot
    /// diverge from a hand-written equivalent, and an ambush is exactly as
    /// debuggable as the trigger an author would otherwise type.
    pub fn to_trigger(&self) -> EnvTrigger {
        let mut effects = self.telegraph.clone();
        for (i, a) in self.actors.iter().enumerate() {
            effects.push(QuestEffect {
                when: None,
                // The ambush declaration, on the beat where the spring becomes
                // real. Only the FIRST, for the reason the field documents: one
                // ambush is one beat, and stamping the line on every generated
                // effect would pad the chronicle and trip `DW0485`.
                happening: if i == 0 { self.happening.clone() } else { None },
                audience: None,
                within: None,
                verb: Verb::SpawnActor { actor: a.clone() },
            });
        }
        for a in &self.actors {
            effects.push(Verb::UnleashActor { actor: a.clone() }.into());
        }
        EnvTrigger {
            id: TriggerId(format!(
                "trigger/{}",
                crate::l10n::local_id(self.id.as_str())
            )),
            at: Some(self.at.clone()),
            // An ambush springs on a body's position or a click on open air; it
            // binds to no pressed block.
            prop: None,
            on: self.trigger.clone(),
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
            once: true,
            // An ambush is a party beat by construction — it springs actors at
            // the room, not a reply to the one who walked in.
            audience: TriggerAudience::Party,
            effects,
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;

/// Validate the stage-5 `ambushes` section (spec-0016 §3), `DW0375`.
///
/// An ambush desugars to an ordinary environment trigger at parse time, so it
/// inherits every trigger diagnostic already in the compiler — id/range checks
/// (`DW0194`), anchor resolution, unknown actor refs, the `use`-on-an-NPC rule
/// (`DW0350`). This function only owns what the sugar itself can get wrong:
/// its own id, and an actor list that does not actually stage an ambush.
///
/// It deliberately does **not** require a `telegraph`. The un-telegraphed
/// ambush is core souls vocabulary — 初见杀 is how a
/// level teaches. What the engine owes the player is counterplay on the retry,
/// which is a geometric question and is proven in `compiler::nav` (`DW0376`).
pub(crate) fn ambush_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, a) in c.quests.content.ambushes.iter().enumerate() {
        if !a.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::AMBUSH_INVALID,
                "quests",
                format!("/content/ambushes/{i}/id"),
                format!(
                    "malformed ambush id `{}` — ambush ids must be lowercase kebab-case with the \
                     `ambush/` prefix (e.g. `ambush/stair-turn`)",
                    a.id
                ),
            ));
        }
        if !seen.insert(a.id.as_str()) {
            d.push(Diagnostic::error(
                codes::AMBUSH_INVALID,
                "quests",
                format!("/content/ambushes/{i}/id"),
                format!(
                    "duplicate ambush id `{}` — rename one so every ambush id is unique (each \
                     desugars to a trigger named after it)",
                    a.id
                ),
            ));
        }
        if a.actors.is_empty() {
            d.push(Diagnostic::error(
                codes::AMBUSH_INVALID,
                "quests",
                format!("/content/ambushes/{i}/actors"),
                format!(
                    "ambush `{}` lists no actors — it would spring nothing. List the actors that \
                     ambush the player, or delete the declaration; a beat that fires and does \
                     nothing is never what was meant.",
                    a.id
                ),
            ));
        }
        let mut dup: BTreeSet<&str> = BTreeSet::new();
        for (j, actor) in a.actors.iter().enumerate() {
            if !dup.insert(actor.as_str()) {
                d.push(Diagnostic::error(
                    codes::AMBUSH_INVALID,
                    "quests",
                    format!("/content/ambushes/{i}/actors/{j}"),
                    format!(
                        "ambush `{}` lists actor `{actor}` twice — `spawn-actor` is idempotent, so \
                         the second one is a silent no-op and the ambush is half the size it \
                         reads as. Declare a second actor instead.",
                        a.id
                    ),
                ));
            }
        }
    }
}
