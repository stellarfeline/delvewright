//! Triggers: the synthesized press answers and the triggers emission writes.

use super::*;

impl<'a> Plan<'a> {
    /// **Every trigger this build emits**: the campaign's own, in declaration
    /// order, then the compiler's press answers ([`PressAnswer`]).
    ///
    /// This is the emission-side counterpart of `QuestsContent::all_triggers` (the
    /// authority an `ambush` desugars into), and it exists for the same reason:
    /// there must be exactly one list, or the sugar acquires a second code path to
    /// drift down. Every place that gives a click a body, a tick clause, a
    /// function, an advancement or a rider tag reads this — so a press answer is
    /// emitted by the code that emits author triggers, and cannot be given a
    /// dialect of its own.
    ///
    /// **Why the press answers are added here rather than in `parse_campaign`**
    /// (where the `ambush` sugar expands). An ambush's strings are the author's
    /// and belong in the campaign's l10n inventory under the desugared trigger's
    /// keys; a press answer's are not. An authored `sealed_hint` is already
    /// inventoried at `fx.….sealed_hint`, and expanding at parse time would move
    /// that key and orphan every sidecar that has it; the compiler's own default
    /// is **chrome**, which must never enter a campaign's inventory at all. The
    /// key contract is a property of the authored document, so the desugar happens
    /// one layer below it — after `localize`, before emission.
    ///
    /// Deterministic: two fixed orders concatenated, no hashing (ADR-0006).
    pub fn emitted_triggers(&self, chrome: &delvewright_dsl::Chrome) -> Vec<EnvTrigger> {
        let mut out = self.campaign.quests.content.triggers.clone();
        out.extend(self.press_answers.iter().map(|p| p.trigger(chrome)));
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

/// The compiler-supplied answer one **pressable body** gives a right-click
/// (DSL v0.11).
///
/// ## Why this is not a field on a verb
///
/// `close-gate` owned `sealed_hint`: its own hitbox fleet, its own advancement,
/// its own actionbar reply, its own baked English. Every one of those is a
/// property of *being a thing a player can press*, and none of them has anything
/// to do with closing a gate — so the second object that needed them, a sealed
/// `shortcut` door, had no surface at all and answered a press with silence,
/// which is exactly the door a souls loop-back invites the party to push on.
/// CLAUDE.md's rule, on this precise case: *a second bespoke field is the
/// defect, not the fix*.
///
/// So a press answer is **not a mechanism**. It is an ordinary
/// [`EnvTrigger`]`{on: use, audience: presser}` carrying an ordinary
/// [`Verb::Narrate`]`{style: actionbar}` — the general "click a thing, run
/// anything" verb, which since DSL v0.11 can reach both the channel and the
/// addressee that the private copy reached. This struct is the *sugar*: the wording
/// and the body it hangs on, lowered by [`PressAnswer::trigger`] into the one path
/// every author-written click already takes. There is no second emitter, no second
/// advancement shape, no second l10n rule and no second diagnostic family.
///
/// ## Lifetime is the body's lifetime
///
/// The synthesized trigger summons nothing: it **rides** the hitboxes the sealed
/// object already owns ([`crate::compiler::pressable::body_at`]). So a `close-gate` seal
/// answers exactly while it is sealed (`open-gate` kills `dw_seal_<safe>`), and a
/// shortcut door answers exactly until it is opened (`shortcut_open_<id>` kills
/// `dw_ws_<safe>`). A door that kept saying it cannot be opened after you opened
/// it would be worse than silence, and nothing has to remember not to do that:
/// there is no answer left to give once the thing you pressed is gone. That is
/// also why the shortcut needs no re-seal reasoning — `DW0372` forbids one.
#[derive(Clone, Debug)]
pub struct PressAnswer {
    /// The anchor of the body this answer hangs on.
    pub anchor: String,
    /// The full id of the trigger this lowers to (`trigger/dw-press-…`).
    pub trigger_id: String,
    /// What owns the body (`close-gate seal` / `shortcut door`), for diagnostics.
    pub owner: &'static str,
    /// The campaign's own wording: an authored `sealed_hint`, l10n-tagged with
    /// its campaign key. A `close-gate` with none has said nothing, and that is
    /// `DW0429`, never a line the compiler supplies.
    pub text: String,
}

/// The `trigger/<local>` id a press answer is synthesized under.
///
/// Two parts carry the collision argument. `dw-` is **reserved** from authored
/// trigger ids (`DW0428`), so a campaign can never write one of these; and
/// `<kind>` separates the two body classes, so a `close-gate` on `anchor/bell`
/// and a `shortcut/bell` — which share nothing but a local name — cannot land on
/// one id and silently become one answer.
fn press_answer_trigger_id(kind: &str, local: &str) -> String {
    format!("trigger/dw-press-{kind}-{local}")
}

/// The local part of an id (`anchor/bell` → `bell`), which is already kebab.
fn local_of(id: &str) -> &str {
    id.split_once('/').map(|(_, r)| r).unwrap_or(id)
}

impl PressAnswer {
    /// This answer lowered into the general verb: a repeatable right-click at the
    /// body's anchor that puts one line on the presser's actionbar.
    ///
    /// `chrome` resolves the compiler's own default into the build's language (a
    /// `--lang` bake ships no language files, so the component's fallback is what
    /// the player reads); an authored line is not chrome and is returned unchanged.
    pub fn trigger(&self, chrome: &delvewright_dsl::Chrome) -> EnvTrigger {
        EnvTrigger {
            id: delvewright_dsl::TriggerId(self.trigger_id.clone()),
            at: Some(delvewright_dsl::AnchorId(self.anchor.clone())),
            // A seal's press answer rides the seal's own bodies; no block.
            prop: None,
            on: delvewright_dsl::TriggerOn::Use,
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
            // A wall is not consumed by being asked: it answers every press.
            once: false,
            audience: delvewright_dsl::TriggerAudience::Presser,
            effects: vec![
                Verb::Narrate {
                    text: chrome.rebind(&self.text),
                    style: Some(delvewright_dsl::NarrateStyle::Actionbar),
                    sound: None,
                }
                .into(),
            ],
        }
    }
}

/// The pressable bodies a press answer can hang on — seals first, then
/// shortcut doors, each in its own planner's order.
///
/// **This list is the class.** A third pressable object gets an answer by joining
/// it, not by growing a field on the verb that owns it. **The campaign authors
/// the wording** for every body in it: the compiler lowers an authored wording
/// onto the general path (a `close-gate`'s `sealed_hint` is an authored answer)
/// and never invents one — a body with neither an authored wording nor a `use`
/// trigger is `DW0429`. A baked default would be the compiler making a design
/// statement on the author's behalf and never telling them it did; an error
/// makes the author say it.
fn press_answer_sites<'p>(
    seal_hints: &'p [SealHintPlan],
    shortcuts: &'p [ShortcutPlan],
) -> Vec<PressSite> {
    let mut out: Vec<PressSite> = seal_hints
        .iter()
        .map(|s| PressSite {
            anchor: s.anchor.clone(),
            trigger_id: press_answer_trigger_id("seal", local_of(&s.anchor)),
            owner: "close-gate seal",
            text: s.text.clone(),
        })
        .collect();
    out.extend(shortcuts.iter().filter_map(|sc| {
        // A door whose sealed side the geometry does not name has no body to hang
        // an answer on; `emit::check_shortcut_sides` (`DW0425`) fails the build
        // before this could matter.
        sc.sealed_side.as_ref()?;
        Some(PressSite {
            anchor: sc.gate_anchor.clone(),
            trigger_id: press_answer_trigger_id("door", local_of(&sc.id)),
            owner: "shortcut door",
            // A shortcut carries no wording field, so there is never an
            // authored wording to lower: its answer is always a trigger.
            text: None,
        })
    }));
    out
}

/// One pressable body and the wording its campaign gave it, if any.
struct PressSite {
    anchor: String,
    trigger_id: String,
    owner: &'static str,
    text: Option<String>,
}

/// **The pressable-body ledger**: every pressable body in the campaign and what
/// owns it — the bodies `DW0429` and the press answers are read from.
///
/// CLAUDE.md: every validation artifact states its binding count. The count
/// that matters is not how many answers the compiler produced — that is zero
/// by design — but how many bodies were **examined**.
pub fn press_answer_bodies(plan: &Plan) -> Vec<(&'static str, String)> {
    press_answer_sites(&plan.seal_hints, &plan.shortcuts)
        .into_iter()
        .map(|s| (s.owner, s.anchor))
        .collect()
}

/// Collect the compiler's press answers: **one per pressable body with an
/// authored wording which the campaign does not answer itself**.
///
/// ## The rule, stated once
///
/// > A sealed body is answered by the campaign — through a `use` trigger, or
/// > an authored wording the compiler lowers onto that same path.
///
/// "The campaign answers it" is `QuestsContent::answers_press_at`, the one
/// predicate `DW0429` also reads, so the refusal and the synthesis can never
/// disagree about what counts as an answer.
pub(super) fn collect_press_answers(
    campaign: &Campaign,
    seal_hints: &[SealHintPlan],
    shortcuts: &[ShortcutPlan],
) -> Vec<PressAnswer> {
    let quests = &campaign.quests.content;
    press_answer_sites(seal_hints, shortcuts)
        .into_iter()
        // The compiler lowers a wording it was GIVEN (an authored `sealed_hint`)
        // and never invents one.
        .filter_map(|site| {
            Some(PressAnswer {
                anchor: site.anchor,
                trigger_id: site.trigger_id,
                owner: site.owner,
                text: site.text?,
            })
        })
        // …and stands down entirely where the campaign answers the press itself.
        .filter(|a| !quests.answers_press_at(&a.anchor))
        .collect()
}
