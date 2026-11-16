//! **An objective keeps the promise its prompt makes** (`DW0860`–`DW0862`).
//!
//! Three playtest findings, on two campaigns, are one defect class: *what the
//! game tells the party and what the machine actually requires are not the same
//! thing*. The instances were fixed one at a time; the class was never built,
//! so every other instance of it stayed in the build waiting for the owner to
//! hit one.
//!
//! | Rule | The finding it generalises |
//! |---|---|
//! | [`DW_CLOCK_UNREAD`] | a beat that can fail the player armed before its own prompt could be read |
//! | [`DW_ADOPTED_CONTAINER_UNMARKED`] | only one of four identical barrels held the item the objective wanted, and nothing told the party which |
//! | [`DW_PROMPT_UNSHOWN`] | an on-screen prompt told the party to return to one place while the objective it described completed somewhere else |
//! | [`DW_ANNOUNCEMENT_EMPTY`] | a declaration that asks for guidance nothing can give — the vacuous shape, refused rather than ignored (spec-0093 §7) |
//!
//! # Why these live together and not next to the verbs they judge
//!
//! Each of them is about the SAME object class — a player-facing promise
//! attached to an objective — and none of them is about the verb that happens to
//! carry it. Split across `loot.rs`, `nav/`, `cast.rs` and `combat.rs` they
//! would be private readings of "what did the party get told", which is
//! exactly the shape [`crate::compiler::cast`] already had to unpick once. One module,
//! one reading of the prompt surface.
//!
//! # What the prompt surface actually is
//!
//! An objective carries two player-facing strings — [`Objective::title`] and
//! [`Objective::hint`] — and its `announcement`, defaulted by the campaign's
//! `guidance` block (spec-0093). What the emitter does with them is the load-bearing fact under
//! these rules: the activation announcement and the completion
//! line are emitted for an **announced** objective — one with a title whose
//! resolved `announcement` is `shown` ([`Objective::announced`]) — and the hint's
//! `tellraw` is nested inside that announcement. So:
//!
//! * an objective that is not announced prints no activation line and no cue;
//!   with a `title` its completion is still told, and with a `title` and a
//!   `hint` it is a hidden task whose hint is on its journal page once it has
//!   started (spec-0137 §3);
//! * an objective with a `hint` and no `title` shows **neither** — the hint is
//!   authored prose the emitter drops on the floor, with nothing anywhere
//!   saying so.
//!
//! # Two obligations and one contradiction
//!
//! [`DW_PROMPT_UNSHOWN`] judges what the document SAYS — a `hint` that asks to be
//! shown and an absent `title` that guarantees it will not be — *a contradiction
//! between two authored fields*. The others **require the campaign to HAVE
//! something**: they are the forcing function `DW0481` applies to the story,
//! turned to face the player.
//!
//! # What these rules deliberately do NOT claim
//!
//! The ledger's general form for the prompt-vs-place finding is *"an objective's
//! prompt names the place and the act that actually complete it"*. That is
//! **not** what [`DW_PROMPT_UNSHOWN`] proves, and the gap is stated rather than
//! papered over. A machine cannot read prose for whether it names the right
//! place; what it can prove is the necessary condition — that the prompt reaches
//! a player at all. The stronger reading was attempted and measured: keying "the
//! place" to the objective's quest's declared `area` produces three findings on
//! a live campaign, and all three are legitimate — a quest booked in one area
//! whose objective is *travelling to* the next one names the destination on
//! purpose. The quest's area is where the beat is booked, not where each
//! objective completes, so it is not a sound proxy, and the sound one (resolving
//! each objective's anchor to its area) is a different subsystem's question.

use delvewright_dsl::Verb;
use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier, Objective, QuestEffect};

delvewright_dsl::dw_code! {
    /// `DW0860`: a **failure clock** — a `begin-stealth` that answers exposure with
    /// an `on_caught` bundle — armed with no prompt before it, or with too little
    /// time between that prompt and the earliest moment it can punish the party.
    ///
    /// Island round 12: the beat armed and the party was punished before the line
    /// telling them what the rules now were had been on screen long enough to read.
    /// The instance was repaired by widening that one beat's grace; the class is
    /// this.
    ///
    /// The arithmetic is stated rather than tuned. `available` is the whole interval
    /// between the last prompt firing and the clock's first bite —
    /// `(arming offset − prompt offset) + grace_ticks`, in ticks, on the arming's own
    /// timeline. `needed` is [`READ_LEAD_TICKS`] plus [`READ_TICKS_PER_CHAR`] per
    /// character of that prompt.
    pub const DW_CLOCK_UNREAD: DwCode = DwCode::new("DW0860", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0861`: a `collect` that **adopts a prefab container** and does not identify
    /// its target to the party — no `title`, so nothing is announced, or no
    /// `item_name`, so the box that is opened holds an anonymous vanilla stack.
    ///
    /// Island round 16: four identical barrels, one of them the objective's, and
    /// nothing told the party which. Adoption is the act that creates the ambiguity:
    /// the compiler's own chest at `anchor` is a new object that appears the tick the
    /// objective activates, whereas an adopted container is — in
    /// [`Objective::Collect::container`]'s own words — *scenery the player has been
    /// walking past since minute one*.
    pub const DW_ADOPTED_CONTAINER_UNMARKED: DwCode = DwCode::new("DW0861", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0862`: an objective authors a `hint` and has no `title`, so the
    /// emitter shows it nowhere and the prompt reaches no player.
    ///
    /// An announced objective's hint is the second line of its announcement;
    /// a hidden task's hint is shown on its journal page, under its title, once
    /// it has started (spec-0137 §3). An objective with no title has neither.
    pub const DW_PROMPT_UNSHOWN: DwCode = DwCode::new("DW0862", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0961`: an objective states `announcement: shown` and has no `title` — an
    /// announcement asked for with nothing to announce (spec-0093 §7).
    ///
    /// The announcement's first line is the title; without one there is nothing
    /// the emitter could print, so the declaration binds to nothing. It is refused
    /// rather than ignored because an inert opt-in is the vacuous shape: the author
    /// believes the party is told, and the party is not.
    pub const DW_ANNOUNCEMENT_EMPTY: DwCode = DwCode::new("DW0961", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1016`: a `highlight` on an act with **nothing to light** (spec-0104
    /// §7.5) — a `collect` whose items fall from a wave (`dropped_by`, so no
    /// container stands), or an `approach`, `step` or `strike-assembly`
    /// trigger (a place, a plate's cell, or an assembly the engine lights as no
    /// one body). An inert declaration is refused, never ignored.
    pub const DW_HIGHLIGHT_INERT: DwCode = DwCode::new("DW1016", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1017`: `compass: shown` on an objective that is **not announced** —
    /// no `title`, or a resolved `announcement` of `hidden` (spec-0104 §6.6).
    /// The compass would point at a thing the player has no name for, and the
    /// line that names what is followed would have nothing to say.
    pub const DW_COMPASS_UNTOLD: DwCode = DwCode::new("DW1017", ExitTier::Build);
}

/// Ticks allowed for a line to appear and the eye to reach it, before any of it
/// is read. One second at 20 tps.
pub const READ_LEAD_TICKS: u32 = 20;

/// Ticks allowed per character of prompt: 2 ticks/char is 10 characters per
/// second, which at the conventional five characters per word is 120 words per
/// minute — a conservative floor for adult silent reading, chosen so that the
/// rule fails a beat only when it is plainly unreadable rather than merely
/// brisk.
pub const READ_TICKS_PER_CHAR: u32 = 2;

/// How long `text` needs to be on screen before a clock may punish the party.
pub fn read_ticks(text: &str) -> u32 {
    READ_LEAD_TICKS + READ_TICKS_PER_CHAR * (text.chars().count() as u32)
}

/// What this module examined, so a green run states its binding rather than
/// leaving a reader to infer it (`CLAUDE.md`: a green gate that binds to nothing
/// is vacuous, not a pass).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PromiseBinding {
    /// Objectives examined, over every quest.
    pub objectives: usize,
    /// Of those, objectives that are announced — have a title and a resolved
    /// `announcement` of `shown` — the population whose hint reaches a screen.
    pub announced: usize,
    /// Of those, `collect` objectives adopting a prefab container —
    /// [`DW_ADOPTED_CONTAINER_UNMARKED`]'s population.
    pub adopted_containers: usize,
    /// Failure clocks examined — [`DW_CLOCK_UNREAD`]'s population.
    pub failure_clocks: usize,
    /// Effect roots enumerated by the clock walk, from the effect-root ledger.
    pub effect_roots: usize,
}

impl PromiseBinding {
    /// The one-line binding statement a run prints, zeroes included — a class
    /// that measured nothing says so rather than passing quietly.
    pub fn line(&self) -> String {
        format!(
            "promise: {} objective(s) examined — {} announced (DW0862/DW0961), {} adopted \
             container(s) (DW0861); {} failure clock(s) (DW0860) over {} effect root(s)",
            self.objectives,
            self.announced,
            self.adopted_containers,
            self.failure_clocks,
            self.effect_roots,
        )
    }
}

/// Run every rule in this module.
pub fn check(c: &Campaign) -> (Vec<Diagnostic>, PromiseBinding) {
    let mut d = Vec::new();
    let mut b = PromiseBinding::default();
    check_objective_prompts(c, &mut d, &mut b);
    check_trigger_props(c, &mut d);
    check_failure_clocks(c, &mut d, &mut b);
    (d, b)
}

/// `DW0964` — a `prop` on a trigger whose event has no cell of its own
/// ([`delvewright_dsl::TriggerOn::places_prop`]).
fn check_trigger_props(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (ti, t) in c.quests.content.triggers.iter().enumerate() {
        let Some(prop) = &t.prop else {
            continue;
        };
        if t.on.places_prop() {
            continue;
        }
        d.push(Diagnostic::error(
            DW_TRIGGER_PROP_INERT,
            "quests",
            format!("/content/triggers/{ti}/prop"),
            format!(
                "trigger `{}` is a `{}` and declares `prop` `{}`. A prop is the block an event \
                 at its own cell acts on — the block a `use` or a `strike` sits on, the target a \
                 `shot` lands on — and this event has no cell of its own to place a block in: \
                 an approach or a step is a body's position, a `strike-npc` is a character and \
                 a `strike-assembly` is an assembly. The block would be placed and nothing \
                 would read it. Remove `prop`, or make the trigger a `use`, a `strike` or a \
                 `shot`.",
                t.id.as_str(),
                t.on.kind(),
                prop.block
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// The three objective-prompt rules
// ---------------------------------------------------------------------------

/// `DW0861`, `DW0862`, `DW0961` — one walk of the objective surface, because
/// all three ask about the same two strings and the visibility beside them.
fn check_objective_prompts(c: &Campaign, d: &mut Vec<Diagnostic>, b: &mut PromiseBinding) {
    let guidance = &c.quests.content.guidance;
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            b.objectives += 1;
            let path = format!("/content/quests/{qi}/objectives/{oi}");
            let id = o.id().as_str();
            let title = o.title().map(str::trim).filter(|s| !s.is_empty());
            let hint = o.hint().map(str::trim).filter(|s| !s.is_empty());
            let announced = o.announced(guidance);
            b.announced += usize::from(announced);

            // DW0862 — a hint the emitter will never show. A hidden task with
            // a title shows its hint on its journal page once it has started
            // (spec-0137 §3); with no title there is no page to put it on.
            if hint.is_some() && title.is_none() {
                d.push(Diagnostic::error(
                    DW_PROMPT_UNSHOWN,
                    "quests",
                    format!("{path}/hint"),
                    format!(
                        "objective `{id}` authors a `hint` and has no `title`, so the party is \
                         shown neither: an announced objective's hint is the second line of its \
                         announcement, and a hidden task's hint is shown on its journal page \
                         under its title once it has started — with no title there is neither, \
                         so this prose is inventoried for translation, rendered into every \
                         language sidecar, and never put on a screen. Give `{id}` a `title`. Do \
                         NOT delete the hint to clear this: the defect is the silence, and \
                         deleting the hint silences the beat instead of fixing it."
                    ),
                ));
            }

            // DW0961 — an announcement asked for with nothing to announce.
            if title.is_none() && o.announcement() == Some(delvewright_dsl::Visibility::Shown) {
                d.push(Diagnostic::error(
                    DW_ANNOUNCEMENT_EMPTY,
                    "quests",
                    format!("{path}/announcement"),
                    format!(
                        "objective `{id}` states `announcement: shown` and has no `title`, so \
                         there is nothing to announce: the `New objective` line is the title, \
                         and without one the emitter prints no line, plays no cue and marks \
                         nothing complete on screen. Give `{id}` a `title`, or remove the \
                         `announcement` field and let the objective be quiet."
                    ),
                ));
            }

            // DW0861 — an adopted container nothing distinguishes.
            if let Objective::Collect {
                holder: Some(container),
                item_name,
                item,
                ..
            } = o
            {
                b.adopted_containers += 1;
                let named = item_name
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|s| !s.is_empty());
                let missing = match (title.is_none(), !named) {
                    (true, true) => Some("neither a `title` nor an `item_name`"),
                    (true, false) => Some("no `title`"),
                    (false, true) => Some("no `item_name`"),
                    (false, false) => None,
                };
                if let Some(missing) = missing {
                    d.push(Diagnostic::error(
                        DW_ADOPTED_CONTAINER_UNMARKED,
                        "quests",
                        path.clone(),
                        format!(
                            "`collect` objective `{id}` adopts the prefab container at \
                             `{container}` and carries {missing}. An adopted container is scenery \
                             the party has been walking past since the beat began, identical to \
                             every other barrel or chest the piece placed, and the compiler adds \
                             nothing to it — so the two things that can tell one box from its \
                             neighbours are the objective's own announcement and the name on the \
                             stack inside. Give `{id}` a `title` and an `item_name` for its \
                             `{item}`. Do NOT reach for `fill_count` instead: padding makes the \
                             right box read full, it does not say which box is right.",
                        ),
                    ));
                }
            }
        }
    }
}

delvewright_dsl::dw_code! {
    /// `DW0964`: a `prop` on a trigger whose event has no cell of its own
    /// (spec-0093 §7; `TriggerOn::places_prop`): an approach, a step, a
    /// `strike-npc` and a `strike-assembly` have no cell to place a block in;
    /// a `use`, a `strike` and a `shot` do. The
    /// field would place a block nothing reads, which is refused rather than
    /// ignored.
    pub const DW_TRIGGER_PROP_INERT: DwCode = DwCode::new("DW0964", ExitTier::Build);
}

/// Where an objective completes: the critical path's step for it when the path
/// performs it, else its own anchor resolved in its quest's area, else the body
/// or wave it is about.
pub(crate) fn objective_place(
    plan: &crate::compiler::plan::Plan,
    quest: &delvewright_dsl::Quest,
    o: &Objective,
) -> Option<[i32; 3]> {
    let c = plan.campaign;
    if let Some(p) = plan
        .objective_steps
        .get(o.id().as_str())
        .and_then(|s| plan.critical_path.get(*s))
        .and_then(|s| s.pos())
    {
        return Some(p);
    }
    let area = plan.quest_area(quest.id.as_str());
    let at = |anchor: &str| {
        area.and_then(|a| plan.point(a, anchor))
            .or_else(|| plan.point_any(anchor))
    };
    match o {
        Objective::Interact { anchor, .. } | Objective::ReachAnchor { anchor, .. } => {
            at(anchor.as_str())
        }
        Objective::Collect {
            id,
            anchor,
            holder: container,
            dropped_by,
            ..
        } => {
            if let Some(f) = plan
                .collect_fills
                .iter()
                .find(|f| f.objective_id == id.as_str())
            {
                return Some(f.cell);
            }
            if let Some(w) = dropped_by {
                return wave_place(plan, w.as_str());
            }
            container
                .as_ref()
                .and_then(|a| at(a.as_str()))
                .or_else(|| at(anchor.as_str()))
        }
        Objective::TalkTo { npc, .. } => c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.as_str())
            .and_then(|n| plan.body_point(delvewright_dsl::BodyRef::Npc(n))),
        Objective::Kill { wave, .. } => wave_place(plan, wave.as_str()),
    }
}

/// A wave's spawn anchor, resolved in the area that spawns it.
fn wave_place(plan: &crate::compiler::plan::Plan, wave_id: &str) -> Option<[i32; 3]> {
    let c = plan.campaign;
    let w = c
        .quests
        .content
        .waves
        .iter()
        .find(|w| w.id.as_str() == wave_id)?;
    let area = crate::compiler::plan::wave_area(c, wave_id)?;
    plan.point(area, w.anchor.as_str())
}

/// An assembly's mark cell.
fn assembly_place(plan: &crate::compiler::plan::Plan, id: &str) -> Option<[i32; 3]> {
    let decl = plan.campaign.quests.content.assembly_decl(id)?;
    plan.point_any(decl.at.anchor.as_str())
        .map(|a| decl.at.cell(a))
}

// ---------------------------------------------------------------------------
// DW0860 — the failure clock
// ---------------------------------------------------------------------------

/// One prompt the party is asked to read, on the arming's own timeline.
#[derive(Debug, Clone)]
struct Prompt {
    /// Tick offset within the bundle at which it fires.
    at: u32,
    /// Position in the bundle's total firing order — the tiebreak when two
    /// prompts fire on the same tick, which is the common case.
    ord: usize,
    /// The text the party must read.
    text: String,
}

/// A failure clock found in a bundle.
#[derive(Debug, Clone)]
struct Clock {
    at: u32,
    ord: usize,
    grace: u32,
    path: String,
}

/// **A failure clock arms only after the prompt that explains it can have been
/// read** (`DW0860`).
///
/// The population is enumerated from the effect-root walk rather than from a
/// remembered list of places effects live — `for_each_effect_root` asserts it
/// reached every root, and its ledger is carried into [`PromiseBinding`].
///
/// Why the other clock surfaces are not judged here, stated so the absence is a
/// decision rather than an oversight:
///
/// * a `timed-gate` arms at **world load**, not at a beat — `timed_gate_setup`
///   runs it from `setup_finish` — so there is no bundle, no ordering, and no
///   prompt that could precede it. What the party is owed there is the ability
///   to *watch* it before committing, which is `DW0388`'s subject, and the ratio
///   of its window, which is `DW0378`'s. Neither is this rule and this rule
///   cannot reach them.
/// * a `volley` and a `collapse` are instantaneous consequences of arriving
///   somewhere, not clocks the party is racing; the souls doctrine
///   (`DW0376`) is explicit that an un-telegraphed ambush is legitimate
///   vocabulary. A clock is different precisely because it keeps punishing.
fn check_failure_clocks(c: &Campaign, d: &mut Vec<Diagnostic>, b: &mut PromiseBinding) {
    let ledger = delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        let mut prompts: Vec<Prompt> = Vec::new();
        let mut clocks: Vec<Clock> = Vec::new();
        let mut ord = 0usize;

        // The bundle's firing order, flattened: a flat member fires at offset 0,
        // a sequence step's members at that step's `at_ticks`. `ord` is the
        // declaration order across the whole bundle, which is the order the
        // emitter writes the commands in and therefore the order one tick's
        // worth of lines reaches the chat.
        for (i, eff) in list.iter().enumerate() {
            match &eff.verb {
                Verb::Sequence { steps } => {
                    for (si, step) in steps.iter().enumerate() {
                        for (ei, inner) in step.effects.iter().enumerate() {
                            note(
                                inner,
                                step.at_ticks,
                                &mut ord,
                                &format!("{}/{i}/steps/{si}/effects/{ei}", site.path),
                                &mut prompts,
                                &mut clocks,
                            );
                        }
                    }
                }
                _ => note(
                    eff,
                    0,
                    &mut ord,
                    &format!("{}/{i}", site.path),
                    &mut prompts,
                    &mut clocks,
                ),
            }
        }

        for clock in &clocks {
            b.failure_clocks += 1;
            // The prompt whose reading the clock actually races is the LAST one
            // to fire at or before the arming — not the longest, and not the sum
            // of the bundle. Summing would fail a beat for prose the party has
            // already read; taking the longest would fail one for a branch
            // variant only some playthroughs ever see (three mutually exclusive
            // retellings of one line is ordinary authoring). The last line before
            // the clock is the instruction, and it is the one still being read
            // when the clock starts.
            let last = prompts
                .iter()
                .filter(|p| p.at < clock.at || (p.at == clock.at && p.ord < clock.ord))
                .max_by_key(|p| (p.at, p.ord));
            let Some(prompt) = last else {
                d.push(Diagnostic::error(
                    DW_CLOCK_UNREAD,
                    site.stage,
                    clock.path.clone(),
                    format!(
                        "this `begin-stealth` arms a failure clock — a player outside every zone \
                         for {} ticks runs its `on_caught` bundle — and nothing in this beat \
                         tells the party the rules changed: no `narrate` fires before it. The \
                         party is punished for a game they were never told they were playing. \
                         Put a `narrate` before the arming in this bundle, saying what is now \
                         being asked of them. Do NOT put the explanation in `on_caught`: that \
                         line is read after the punishment, which is the defect rather than the \
                         fix.",
                        clock.grace,
                    ),
                ));
                continue;
            };
            let available = clock.at.saturating_sub(prompt.at) + clock.grace;
            let needed = read_ticks(&prompt.text);
            if available < needed {
                let chars = prompt.text.chars().count();
                d.push(Diagnostic::error(
                    DW_CLOCK_UNREAD,
                    site.stage,
                    clock.path.clone(),
                    format!(
                        "this `begin-stealth` can punish the party {available} ticks after the \
                         line that explains it, and that line takes {needed} ticks to read \
                         ({chars} characters at {READ_TICKS_PER_CHAR} ticks each, after \
                         {READ_LEAD_TICKS} ticks for it to appear). A clock that bites while its \
                         own instruction is still on screen fails the party for not having read \
                         fast enough. Raise `grace_ticks` above {needed}, or move the arming into \
                         a later `sequence` step so the reading happens before the clock starts. \
                         Do NOT shorten the line to fit the clock — the line is what makes the \
                         beat playable.",
                    ),
                ));
            }
        }
    });
    b.effect_roots = ledger.roots_enumerated;
}

/// Record one effect as a prompt, a clock, or neither.
fn note(
    eff: &QuestEffect,
    at: u32,
    ord: &mut usize,
    path: &str,
    prompts: &mut Vec<Prompt>,
    clocks: &mut Vec<Clock>,
) {
    let here = *ord;
    *ord += 1;
    match &eff.verb {
        // Every narrate channel is a prompt: chat, title, subtitle, actionbar and
        // art all put authored words in front of the party. The channel changes
        // where the words sit, never whether they have to be read.
        Verb::Narrate(delvewright_dsl::Narration { text, .. }) => {
            prompts.push(Prompt {
                at,
                ord: here,
                text: text.clone(),
            });
        }
        Verb::BeginStealth {
            on_caught,
            grace_ticks,
            ..
        } if !on_caught.is_empty() => clocks.push(Clock {
            at,
            ord: here,
            grace: *grace_ticks,
            path: path.to_string(),
        }),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// DW1017 — the compass follows only what the party is told (spec-0104 §6.6)
// ---------------------------------------------------------------------------

/// What [`check_compass`] judged, stated whichever way it went.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompassBinding {
    /// Objectives examined.
    pub objectives: usize,
    /// Of those, tracked — announced, with a resolved `compass` of `shown`.
    pub tracked: usize,
    /// Of the tracked, by their own `compass` field.
    pub by_field: usize,
    /// Of the tracked, by the campaign's `guidance.compass`.
    pub by_default: usize,
    /// `compass: shown` on an unannounced objective, refused.
    pub refused: usize,
}

impl CompassBinding {
    /// The one line a run prints about the compass.
    pub fn line(&self) -> String {
        format!(
            "compass binding: {} tracked objective(s) of {} — {} by their own field, {} by the \
             campaign default; {} refused",
            self.tracked, self.objectives, self.by_field, self.by_default, self.refused
        )
    }
}

/// **The compass follows announced objectives** (`DW1017`): an objective that
/// states `compass: shown` and is not announced is refused.
pub fn check_compass(c: &Campaign) -> (Vec<Diagnostic>, CompassBinding) {
    let guidance = &c.quests.content.guidance;
    let mut d = Vec::new();
    let mut b = CompassBinding::default();
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            b.objectives += 1;
            if o.tracked(guidance) {
                b.tracked += 1;
                if o.compass().is_some() {
                    b.by_field += 1;
                } else {
                    b.by_default += 1;
                }
                continue;
            }
            if o.compass() != Some(delvewright_dsl::Visibility::Shown) || o.announced(guidance) {
                continue;
            }
            b.refused += 1;
            let id = o.id().as_str();
            let why = if o.title().is_none_or(|t| t.trim().is_empty()) {
                "has no `title`"
            } else if o.announcement().is_some() {
                "states `announcement: hidden`"
            } else {
                "falls under the campaign's `guidance.announcements: hidden`"
            };
            d.push(Diagnostic::error(
                DW_COMPASS_UNTOLD,
                "quests",
                format!("/content/quests/{qi}/objectives/{oi}/compass"),
                format!(
                    "objective `{id}` states `compass: shown` and is not announced — it {why}. \
                     The compass follows only an objective the party has been told about: it \
                     would point at a thing the player has no name for, and the line that says \
                     what is followed would have nothing to say. Announce `{id}` (give it a \
                     `title` and let its announcement show), or remove the `compass` field."
                ),
            ));
        }
    }
    (d, b)
}

// ---------------------------------------------------------------------------
// DW1016 — a highlight lights a thing (spec-0104 §7.5)
// ---------------------------------------------------------------------------

/// What [`check_highlights`] judged, stated whichever way it went.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HighlightBinding {
    /// Acts that admit a highlight: `interact`, `collect`, `talk-to` and
    /// `kill` objectives, and every trigger.
    pub acts: usize,
    /// Highlights declared.
    pub highlights: usize,
    /// Of those, on a block (an `interact`, a `collect`'s container, a `use` or
    /// `strike` trigger).
    pub on_block: usize,
    /// Of those, on a body (a `talk-to`'s NPC, a `kill`'s wave, a
    /// `strike-npc`'s NPC).
    pub on_body: usize,
    /// Refused: nothing to light.
    pub refused: usize,
}

impl HighlightBinding {
    /// The one line a run prints about highlights.
    pub fn line(&self) -> String {
        format!(
            "highlight binding: {} highlight(s) over {} act(s) — {} on a block, {} on a body; {} \
             refused",
            self.highlights, self.acts, self.on_block, self.on_body, self.refused
        )
    }
}

/// **A highlight lights a thing** (`DW1016`): a `highlight` on a `collect`
/// dropped by a wave, or on an `approach`, `step` or `strike-assembly`
/// trigger, is refused.
pub fn check_highlights(c: &Campaign) -> (Vec<Diagnostic>, HighlightBinding) {
    use delvewright_dsl::TriggerOn;
    let mut d = Vec::new();
    let mut b = HighlightBinding::default();
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            if matches!(o, Objective::ReachAnchor { .. }) {
                continue;
            }
            b.acts += 1;
            if o.highlight().is_none() {
                continue;
            }
            b.highlights += 1;
            match o {
                Objective::Collect {
                    dropped_by: Some(wave),
                    ..
                } => {
                    b.refused += 1;
                    let id = o.id().as_str();
                    d.push(Diagnostic::error(
                        DW_HIGHLIGHT_INERT,
                        "quests",
                        format!("/content/quests/{qi}/objectives/{oi}/highlight"),
                        format!(
                            "`collect` objective `{id}` declares `highlight` and its items fall \
                             from wave `{}`: no container stands for the glow to light, so the \
                             declaration lights nothing. Remove `highlight`, or give the \
                             objective a thing to light (the wave's `kill` objective can carry \
                             the highlight on its bodies).",
                            wave.as_str()
                        ),
                    ));
                }
                Objective::Collect { gathered: true, .. } => {
                    b.refused += 1;
                    let id = o.id().as_str();
                    d.push(Diagnostic::error(
                        DW_HIGHLIGHT_INERT,
                        "quests",
                        format!("/content/quests/{qi}/objectives/{oi}/highlight"),
                        format!(
                            "`collect` objective `{id}` declares `highlight` and its item is \
                             `gathered`: the world supplies it and the engine places nothing, \
                             so no block or body stands for the glow to light. Remove \
                             `highlight`, or give the glow a thing to light (an `interact` on \
                             the place where the item is got can carry it)."
                        ),
                    ));
                }
                Objective::TalkTo { .. } | Objective::Kill { .. } => b.on_body += 1,
                _ => b.on_block += 1,
            }
        }
    }
    for (ti, t) in c.quests.content.triggers.iter().enumerate() {
        b.acts += 1;
        if t.highlight.is_none() {
            continue;
        }
        b.highlights += 1;
        let what = match &t.on {
            TriggerOn::Use | TriggerOn::Strike | TriggerOn::Shot => {
                b.on_block += 1;
                continue;
            }
            TriggerOn::StrikeNpc { .. }
            | TriggerOn::StrikeActor { .. }
            | TriggerOn::UseActor { .. } => {
                b.on_body += 1;
                continue;
            }
            TriggerOn::UseItem { .. } => "a `use-item` watches a place a body stands in",
            TriggerOn::BodyEnters { .. } => "a `body-enters` watches a box bodies walk into",
            TriggerOn::Approach { .. } => "an `approach` watches a place a body stands in",
            TriggerOn::Step => "a `step` watches the cell a body steps into",
            TriggerOn::StrikeAssembly { .. } => {
                "a `strike-assembly` strikes an assembly of display entities the engine lights \
                 as no one body"
            }
        };
        b.refused += 1;
        d.push(Diagnostic::error(
            DW_HIGHLIGHT_INERT,
            "quests",
            format!("/content/triggers/{ti}/highlight"),
            format!(
                "trigger `{}` declares `highlight`, and {what}: there is no block or body for the \
                 glow to light. Remove `highlight`, or make the act one with a thing to light (a \
                 `use` or `strike` on a block, a `strike-npc` on a person).",
                t.id.as_str()
            ),
        ));
    }
    // spec-0117 §3b: every other pressable, and a stake's place, lights a block.
    let q = &c.quests.content;
    let others = q
        .shortcuts
        .iter()
        .map(|s| s.highlight.is_some())
        .chain(q.shops.iter().map(|s| s.highlight.is_some()))
        .chain(q.stakes.iter().map(|s| s.highlight.is_some()))
        .chain(
            q.traps
                .iter()
                .filter_map(|t| t.disarm.as_ref())
                .map(|d| d.highlight.is_some()),
        )
        .chain(
            q.timed_gates
                .iter()
                .filter_map(|g| g.disarm.as_ref())
                .map(|d| d.highlight.is_some()),
        );
    let mut bonfires = Vec::new();
    delvewright_dsl::for_each_campaign_effect(c, &mut |_, _, eff| {
        if let delvewright_dsl::Verb::Bonfire { highlight, .. } = &eff.verb {
            bonfires.push(highlight.is_some());
        }
    });
    for lit in others.chain(bonfires) {
        b.acts += 1;
        if lit {
            b.highlights += 1;
            b.on_block += 1;
        }
    }
    (d, b)
}
