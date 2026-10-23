//! Triggers: the triggers emission writes, and the pressables the plan places.

use super::*;
use delvewright_dsl::{DwCode, ExitTier};

impl<'a> Plan<'a> {
    /// **Every trigger this build emits**: the campaign's own, in declaration
    /// order, then every pressable's declared answers
    /// ([`crate::compiler::pressable::AnswerPlan`], spec-0117), then every
    /// body's declared answers ([`crate::compiler::bodies::BodyAnswer`],
    /// spec-0119).
    ///
    /// This is the emission-side counterpart of `QuestsContent::all_triggers`
    /// (the authority an `ambush` desugars into), and it exists for the same
    /// reason: there must be exactly one list. Every place that gives a click a
    /// body, a tick clause, a function, an advancement or a rider tag reads this
    /// — so an answer is emitted by the code that emits author triggers, and
    /// cannot be given a dialect of its own.
    ///
    /// The answers are added here rather than at parse time because their
    /// strings are already inventoried under the pressable's own keys
    /// (`obj.….answers.…`, `trigger.….answers.…`); the desugar happens one layer
    /// below the authored document — after `localize`, before emission.
    ///
    /// Deterministic: fixed orders concatenated, no hashing (ADR-0006). The
    /// `chrome` parameter is kept for the localized-build path: no answer is
    /// chrome, so none is rebound.
    pub fn emitted_triggers(&self, _chrome: &delvewright_dsl::Chrome) -> Vec<EnvTrigger> {
        let mut out = self.campaign.quests.content.triggers.clone();
        out.extend(self.answers().iter().map(|a| a.trigger()));
        out.extend(
            crate::compiler::bodies::answers(self.campaign)
                .iter()
                .map(|a| a.trigger()),
        );
        out
    }

    /// [`Self::emitted_triggers`] for a consumer that asks *which triggers exist,
    /// where, and of what kind* and never reads what they say — the hitbox proofs
    /// (`DW0422`/`DW0426`) and the affordance ledger.
    ///
    /// The build language only ever decides which rendition of a **chrome**
    /// string rides a component as its fallback, so a body question cannot depend
    /// on it. Spelling that out here is what keeps those consumers from having to
    /// thread a `Chrome` they would not use.
    pub fn emitted_triggers_unlocalized(&self) -> Vec<EnvTrigger> {
        self.emitted_triggers(&delvewright_dsl::Chrome::default())
    }
}

impl<'a> Plan<'a> {
    /// **Every interaction point this build places** (spec-0117 §3a): derived
    /// once, on first read, from the finished plan.
    pub fn presses(&self) -> &[crate::compiler::pressable::PressPlan] {
        self.presses
            .get_or_init(|| crate::compiler::pressable::derive(self))
    }

    /// **Every declared answer this build emits** (spec-0117 §5.2): derived
    /// once, on first read, from [`Self::presses`].
    pub fn answers(&self) -> &[crate::compiler::pressable::AnswerPlan] {
        self.answer_plans
            .get_or_init(|| crate::compiler::pressable::derive_answers(self))
    }
}

delvewright_dsl::dw_code! {
    /// `DW0429` (spec-0117 §6): **a sealed gate with no answer.** Every
    /// `close-gate` target and every shortcut `gate` owes an answer trigger — a
    /// `use` trigger with `serves: {gate: <that anchor>}` anchored either at a
    /// cell **inside** the region that holds a block while sealed (the gate's own
    /// answer: its body is that block under a click box) or, for a `close-gate`
    /// target, at a cell **face-adjacent** to the region with its own `prop` (a
    /// knocker). A shortcut's door is answered by its own leaf, pressed from the
    /// sealed side as the presser (the far side hears the shortcut's
    /// `answers.not_yet` from the same leaf). The compiler never writes the
    /// words: a push on a sealed door is otherwise silent.
    pub const SEALED_GATE_UNANSWERED: DwCode = DwCode::new("DW0429", ExitTier::Build);
}
