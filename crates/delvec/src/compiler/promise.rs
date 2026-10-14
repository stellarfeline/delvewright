//! **An objective keeps the promise its prompt makes** (`DW0860`–`DW0863`).
//!
//! Four playtest findings, on two campaigns, are one defect class: *what the
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
//! | [`DW_FIGHT_UNSIGNED`] | a defence wave gave the party no guidance to where the attackers were, so the fight could not be found |
//! | [`DW_ANNOUNCEMENT_EMPTY`] / [`DW_MARKER_INERT`] | a declaration that asks for guidance nothing can give — the vacuous shape, refused rather than ignored (spec-0093 §7) |
//! | [`DW_PRESSABLE_UNSEEN`] | a quiet delve's slate was an invisible hitbox on an empty cell, and the party could not find the thing to press (spec-0093 §7) |
//!
//! # Why these live together and not next to the verbs they judge
//!
//! Each of the four is about the SAME object class — a player-facing promise
//! attached to an objective — and none of them is about the verb that happens to
//! carry it. Split across `loot.rs`, `nav/`, `cast.rs` and `combat.rs` they
//! would be four private readings of "what did the party get told", which is
//! exactly the shape [`crate::compiler::cast`] already had to unpick once. One module,
//! one reading of the prompt surface.
//!
//! # What the prompt surface actually is
//!
//! An objective carries two player-facing strings — [`Objective::title`] and
//! [`Objective::hint`] — and two visibilities, its `marker` and its
//! `announcement`, each defaulted by the campaign's `guidance` block
//! (spec-0093). What the emitter does with them is the load-bearing fact under
//! three of these four rules: the activation announcement and the completion
//! line are emitted for an **announced** objective — one with a title whose
//! resolved `announcement` is `shown` ([`Objective::announced`]) — and the hint's
//! `tellraw` is nested inside that announcement. So:
//!
//! * an objective that is not announced prints **nothing at all** — no chat
//!   line, no cue sound — and its wayfinding marker, when it has one, is
//!   summoned nameless (`emit::marker_name_fields` refuses to surface a raw
//!   `obj/…` id);
//! * an objective with a `hint` that is not announced shows **neither** — the
//!   hint is authored prose the emitter drops on the floor, with nothing
//!   anywhere saying so.
//!
//! # Three obligations and one contradiction
//!
//! [`DW_PROMPT_UNSHOWN`] judges what the document SAYS — a `hint` that asks to be
//! shown and an absent `title` that guarantees it will not be — *a contradiction
//! between two authored fields*. The other three **require the campaign to HAVE
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
    /// `DW0862`: an objective authors a `hint` and is **not announced** — it has
    /// no `title`, or its resolved `announcement` is `hidden` (spec-0093) — so the
    /// emitter shows **neither** and the prompt reaches no player.
    ///
    /// The activation announcement is emitted only for an announced objective, and
    /// the hint's `tellraw` is nested inside that guard — so a hint on an
    /// unannounced objective is prose that is inventoried for translation,
    /// rendered into every language sidecar, and never once put on a screen.
    /// Nothing else in the toolchain says so: it is not a warning, not a lint, and
    /// the l10n inventory counts it as a live string.
    pub const DW_PROMPT_UNSHOWN: DwCode = DwCode::new("DW0862", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0863`: an act on a walked path the party cannot find — neither
    /// **announced with a hint** nor **in sight** from the cell the party stands
    /// on before it (and, for a `kill`, its wave within reach of that cell)
    /// (spec-0104 §3, widening spec-0093 §5).
    ///
    /// Bell round 6: a defence wave gave the party no guidance to where the
    /// attackers were. The Stranding round 1: steps with no hint went unseen, and
    /// a fight's bodies were seated out of sight. An act is every objective step
    /// and every `use`/`strike` trigger the path performs; what the party is owed
    /// for each is the same whatever the kind: it can see the act from where it
    /// stands, or the objective's own lines say where it is. A trigger has no
    /// lines, so sight is its only arm.
    ///
    /// Judged at the build, where places and blocks exist (`emit::build`).
    pub const DW_FIGHT_UNSIGNED: DwCode = DwCode::new("DW0863", ExitTier::Build);
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
    /// `DW0962`: `marker` declared on an `interact` that declares a `prop` — the prop
    /// is the affordance and no marker is ever summoned beside it, so the field
    /// declares nothing (spec-0093 §7).
    ///
    /// `shown` is a lie (nothing glows) and `hidden` is redundant (nothing did);
    /// either way the author wrote a value the world will not reflect.
    pub const DW_MARKER_INERT: DwCode = DwCode::new("DW0962", ExitTier::Build);
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
    /// Of those, objectives whose kind summons a marker and whose resolved
    /// `marker` is `shown`.
    pub marked: usize,
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
            "promise: {} objective(s) examined — {} announced, {} marked (DW0862/DW0961/DW0962), \
             {} adopted container(s) (DW0861); {} failure clock(s) (DW0860) over {} effect root(s)",
            self.objectives,
            self.announced,
            self.marked,
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

/// `DW0964` — a `prop` on a trigger that is not a `use`.
fn check_trigger_props(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (ti, t) in c.quests.content.triggers.iter().enumerate() {
        let Some(prop) = &t.prop else {
            continue;
        };
        if matches!(
            t.on,
            delvewright_dsl::TriggerOn::Use | delvewright_dsl::TriggerOn::Strike
        ) {
            continue;
        }
        d.push(Diagnostic::error(
            DW_TRIGGER_PROP_INERT,
            "quests",
            format!("/content/triggers/{ti}/prop"),
            format!(
                "trigger `{}` is a `{}` and declares `prop` `{}`. A prop is the visible object a \
                 click acts on — the block a `use` or a `strike` sits on at its own cell — and \
                 this event has no cell of its own to place a block in: an approach is a body's \
                 position, a `strike-npc` is a character and a `strike-assembly` is an assembly. \
                 The block would be placed and nothing would read it. Remove `prop`, or make the \
                 trigger a `use` or a `strike`.",
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

/// `DW0861`, `DW0862`, `DW0961`, `DW0962` — one walk of the objective surface,
/// because all four ask about the same two strings and the two visibilities
/// beside them.
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
            b.marked += usize::from(o.marker_shown(guidance));

            // DW0862 — a hint the emitter will never show.
            if hint.is_some() && !announced {
                let why = if title.is_none() {
                    "has no `title`".to_string()
                } else if o.announcement().is_some() {
                    "states `announcement: hidden`".to_string()
                } else {
                    "falls under the campaign's `guidance.announcements: hidden`".to_string()
                };
                d.push(Diagnostic::error(
                    DW_PROMPT_UNSHOWN,
                    "quests",
                    format!("{path}/hint"),
                    format!(
                        "objective `{id}` authors a `hint` and is not announced — it {why} — so \
                         the party is shown neither: the activation announcement is emitted \
                         only for an announced objective and the hint's line is nested inside \
                         it, so this prose is inventoried for translation, rendered into every \
                         language sidecar, and never put on a screen. Give `{id}` a `title` and \
                         let it be announced — the hint is the second line of an announcement, \
                         not an announcement. Do NOT delete the hint to clear this: the defect \
                         is the silence, and deleting the hint silences the beat instead of \
                         fixing it."
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

            // DW0962 — a marker declared where none is ever summoned.
            if let Objective::Interact {
                prop: Some(prop),
                marker: Some(marker),
                ..
            } = o
            {
                d.push(Diagnostic::error(
                    DW_MARKER_INERT,
                    "quests",
                    format!("{path}/marker"),
                    format!(
                        "objective `{id}` declares `marker: {}` beside a `prop` (`{}`). The prop \
                         block IS the affordance and no marker is ever summoned beside one, so \
                         the field declares nothing the world will reflect. Remove `marker`, or \
                         remove the `prop` and let the compiler's lantern stand there.",
                        match marker {
                            delvewright_dsl::Visibility::Shown => "shown",
                            delvewright_dsl::Visibility::Hidden => "hidden",
                        },
                        prop.block
                    ),
                ));
            }

            // DW0861 — an adopted container nothing distinguishes.
            if let Objective::Collect {
                container: Some(container),
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
    /// `DW0963`: **a thing to act on that nothing shows** (spec-0093 §7). An
    /// `interact` that is not marked — its resolved `marker` is `hidden` — with no
    /// `prop`, or a `use`/`strike` trigger with no `prop` whose anchor is a point
    /// in open air, whose hitbox cells hold air in the assembled world: the
    /// player is asked to act on a point in empty space.
    ///
    /// The hitbox a player presses is a `minecraft:interaction`, which is
    /// invisible, and is never the object. A quiet act sits on a **visible,
    /// authored object at its cell** — a block the `prop` places or the piece
    /// authored, a lever or a bell or a lamp or a winch, anything that reads as
    /// the thing. Where vanilla reports the object's use — a lever flipped, a
    /// button pressed, a bell rung — that report is the detection and no hitbox
    /// exists; where it does not, the hitbox is fitted over the visible object
    /// as its hit area. A trigger that rides a seal's or a shortcut's own
    /// bodies, or arms a gate region's shell, sits on that structure. Build
    /// tier, over the settled bytes, beside `DW0863`.
    pub const DW_PRESSABLE_UNSEEN: DwCode = DwCode::new("DW0963", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0964`: a `prop` on a trigger whose event is not a `use` (spec-0093 §7).
    /// A prop is the object a right-click acts on; a strike is a left-click
    /// vanilla reports no block for, and an approach, a `strike-npc` and a
    /// `strike-assembly` have no cell of their own to place a block in. The
    /// field would place a block nothing reads, which is refused rather than
    /// ignored.
    pub const DW_TRIGGER_PROP_INERT: DwCode = DwCode::new("DW0964", ExitTier::Build);
}

/// What [`check_pressables_visible`] examined, stated whichever way it went.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PressableBinding {
    /// `interact` objectives examined, over every quest.
    pub interacts: usize,
    /// Click triggers (`use` / `strike`) examined.
    pub triggers: usize,
    /// Of both, bound to a vanilla block a hand presses — no hitbox at all.
    pub block_bound: usize,
    /// Of the interacts, marked — the lantern shows them.
    pub marked: usize,
    /// Of both, carrying a `prop` vanilla does not report the use of — the block
    /// is placed and the hitbox stands in it.
    pub with_prop: usize,
    /// Of the triggers, standing on a seal's, a shortcut's or a gate region's
    /// own structure.
    pub on_a_structure: usize,
    /// Strikes carried by an NPC's own hitbox, the NPC being the visible thing.
    pub on_an_npc: usize,
    /// Of the rest, standing in a cell the assembled world fills with a block.
    pub on_a_block: usize,
    /// Of the rest, whose anchor resolves to no cell (another rule's refusal).
    pub unplaced: usize,
}

impl PressableBinding {
    /// The one line a build prints about this rule.
    pub fn line(&self) -> String {
        format!(
            "DW0963 binding: {} `interact` objective(s) and {} click trigger(s); {} bound to a \
             block a hand presses, {} marked, {} with a prop vanilla does not report, {} on a \
             seal, shortcut or gate shell, {} on an NPC's own hitbox, {} on an authored block, \
             {} unplaced",
            self.interacts,
            self.triggers,
            self.block_bound,
            self.marked,
            self.with_prop,
            self.on_a_structure,
            self.on_an_npc,
            self.on_a_block,
            self.unplaced
        )
    }
}

/// **A pressable thing stands on something the player can see** (`DW0963`):
/// every `interact` that is not marked and every `use`/`strike` trigger is a
/// vanilla block a hand presses, or stands on a structure the compiler arms, or
/// stands in a cell the piece filled. Reads the settled block map for the last
/// arm: the feet cell and the head cell the hitbox occupies at the anchor,
/// either holding a non-air block, is the thing the player presses; both air
/// is a point in space.
pub fn check_pressables_visible(
    plan: &crate::compiler::plan::Plan,
    blocks: &crate::compiler::blockstate::BlockMap,
) -> (
    PressableBinding,
    Result<(), crate::compiler::failure::Failure>,
) {
    use crate::compiler::pressable::Body;
    let c = plan.campaign;
    let guidance = &c.quests.content.guidance;
    let mut b = PressableBinding::default();
    let mut verdict: Result<(), crate::compiler::failure::Failure> = Ok(());
    let cells_hold_a_block = |pos: [i32; 3]| {
        [pos, [pos[0], pos[1] + 1, pos[2]]]
            .iter()
            .filter_map(|cell| blocks.get(cell).map(|s| s.as_str()))
            .any(|name| !delvewright_dsl::blockshape::is_air(name))
    };
    let refuse = |verdict: &mut Result<(), crate::compiler::failure::Failure>,
                  what: String,
                  how: &str,
                  anchor: &str,
                  pos: [i32; 3],
                  remedy: &str| {
        if verdict.is_ok() {
            *verdict = Err(crate::compiler::failure::Failure {
                code: DW_PRESSABLE_UNSEEN,
                message: format!(
                    "{what} asks the party to press a point in empty space: {how}, and the \
                     cells its hitbox occupies at `{anchor}` ({pos:?} and the cell above) hold \
                     air in the assembled world. The hitbox is a `minecraft:interaction`, which \
                     nobody can see. {remedy} Do NOT move the anchor into a wall to satisfy \
                     this: the cell a body stands in to press is the cell this reads."
                ),
            });
        }
    };
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            let Objective::Interact {
                id, anchor, prop, ..
            } = o
            else {
                continue;
            };
            b.interacts += 1;
            if crate::compiler::pressable::interact_block(o).is_some() {
                b.block_bound += 1;
                continue;
            }
            if prop.is_some() {
                b.with_prop += 1;
                continue;
            }
            if o.marker_shown(guidance) {
                b.marked += 1;
                continue;
            }
            let area = plan.quest_area(q.id.as_str());
            let Some(pos) = area
                .and_then(|a| plan.point(a, anchor.as_str()))
                .or_else(|| plan.point_any(anchor.as_str()))
            else {
                b.unplaced += 1;
                continue;
            };
            if cells_hold_a_block(pos) {
                b.on_a_block += 1;
                continue;
            }
            let how = if o.marker().is_some() {
                "its marker is hidden by its own `marker: hidden` and it declares no `prop`"
            } else {
                "its marker is hidden by the campaign's `guidance.markers: hidden` and it \
                 declares no `prop`"
            };
            refuse(
                &mut verdict,
                format!(
                    "`interact` objective `{}` (quest {qi}, objective {oi})",
                    id.as_str()
                ),
                how,
                anchor.as_str(),
                pos,
                &format!(
                    "Give `{}` a `prop` — any block that reads as the thing (a lever, a bell, a \
                     lamp, a winch); a lever, a button or a bell is also its own detector, since \
                     vanilla reports its use — or author a block at `{}` in the piece, or show \
                     its marker. If the object does not explain itself, the objective's `hint` \
                     names it.",
                    id.as_str(),
                    anchor.as_str()
                ),
            );
        }
    }
    for (ti, t) in c.quests.content.triggers.iter().enumerate() {
        use delvewright_dsl::TriggerOn;
        if !matches!(t.on, TriggerOn::Use | TriggerOn::Strike) {
            continue;
        }
        b.triggers += 1;
        match crate::compiler::pressable::trigger_body(plan, t) {
            Body::Block { .. } => {
                b.block_bound += 1;
                continue;
            }
            Body::Rides { .. } | Body::Region(_) => {
                b.on_a_structure += 1;
                continue;
            }
            Body::Nothing => {
                b.unplaced += 1; // `DW0426`'s refusal
                continue;
            }
            Body::Point(pos) => {
                if t.prop.is_some() {
                    b.with_prop += 1;
                    continue;
                }
                // A strike at an NPC's own stand rides the NPC's hitbox: the
                // body is the visible thing (the emitter's one rule).
                if crate::compiler::emit::npc_ridden_by(plan, t).is_some() {
                    b.on_an_npc += 1;
                    continue;
                }
                if cells_hold_a_block(pos) {
                    b.on_a_block += 1;
                    continue;
                }
                let Some(at) = t.at_anchor() else {
                    continue;
                };
                let remedy = match t.on {
                    TriggerOn::Use => format!(
                        "Give `{}` a `prop` — any block that reads as the thing; a lever, a \
                         button or a bell is also its own detector, since vanilla reports its \
                         use — or author a block at `{at}` in the piece.",
                        t.id.as_str()
                    ),
                    _ => format!(
                        "A strike has no vanilla block signal, so the hit area stays fitted over \
                         the object: author the thing to strike at `{at}` in the piece, or make \
                         `{}` a `use` with a `prop`.",
                        t.id.as_str()
                    ),
                };
                refuse(
                    &mut verdict,
                    format!(
                        "`{}` trigger `{}` (trigger {ti})",
                        t.on.kind(),
                        t.id.as_str()
                    ),
                    "it declares no `prop` and its anchor is a point in open air",
                    at,
                    pos,
                    &remedy,
                );
            }
        }
    }
    (b, verdict)
}

// ---------------------------------------------------------------------------
// DW0863 — an act the party cannot find (spec-0104 §3)
// ---------------------------------------------------------------------------

/// The one reach sight is given (spec-0104 §2.5): the farthest the pinned
/// client draws a body of a block or less at the default *Entity Distance*
/// (`64 × size × viewScale`, size clamped to at least 1), used for blocks as
/// well.
pub const SIGHT_BLOCKS: f64 = 64.0;

/// The eye of a body standing on a cell: [`crate::compiler::nav::cell_center`]
/// plus the standing eye height.
const EYE_HEIGHT: f64 = 1.62;

/// What [`check_acts_found`] examined, stated whichever way it went.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActBinding {
    /// Acts judged, counted once per walk they stand on.
    pub acts: usize,
    /// Walks taken: the critical path and every realised branch.
    pub walks: usize,
    /// Of the acts, announced with a `hint` — found by the objective's own lines.
    pub announced: usize,
    /// Of the acts, in sight from where the party stands.
    pub in_sight: usize,
    /// Of those, `kill`s whose wave is also within reach.
    pub kills_in_reach: usize,
    /// Of the acts, triggers — judged by sight alone, having no lines.
    pub triggers: usize,
    /// `kill` objectives on no walk, judged from the acts that fire their wave.
    pub kills_off_walk: usize,
    /// Acts refused.
    pub refused: usize,
}

impl ActBinding {
    /// The one line a build prints about this rule.
    pub fn line(&self) -> String {
        format!(
            "DW0863 binding: {} act(s) over {} walk(s) — {} announced with a hint, {} in sight \
             ({} kill(s) also in reach), {} trigger(s) judged by sight alone; {} refused; {} \
             kill(s) on no walk judged from the acts that fire their wave",
            self.acts,
            self.walks,
            self.announced,
            self.in_sight,
            self.kills_in_reach,
            self.triggers,
            self.refused,
            self.kills_off_walk
        )
    }
}

/// One site that fires a wave's `spawn-wave`, as the reach rule reads it.
struct FiringSite {
    /// Where the bundle hangs, for the message.
    path: String,
    /// The cells the party stands at when it fires; `None` when the root places
    /// the party nowhere this rule can name.
    places: Option<Vec<[i32; 3]>>,
}

/// **Why an act is not found from one vantage** (spec-0104 §3, arm 2): the
/// reasons its target is out of sight from a body standing on `from` — past
/// [`SIGHT_BLOCKS`], or behind the first cell that stops the line — and, when
/// `reach` is given (a `kill`'s wave reach and where it comes from), whether
/// the target is past it. Empty when the act is found.
pub fn act_unfound(
    world: &crate::compiler::nav::World,
    from: [i32; 3],
    target: [i32; 3],
    body: bool,
    reach: Option<(f64, &str)>,
) -> Vec<String> {
    let mut why = Vec::new();
    if let Some(u) = unseen(world, from, target, body) {
        why.push(format!("its target {target:?} is {u}"));
    }
    if let Some((r, src)) = reach {
        let d = dist(from, target);
        if d > r {
            why.push(format!(
                "its wave's anchor {target:?} is {d:.1} blocks away, past the wave's reach of \
                 {r} blocks ({src}), so the bodies the party sees do not come to it"
            ));
        }
    }
    why
}

/// Why a target is not in sight from a cell, or `None` when it is.
fn unseen(
    world: &crate::compiler::nav::World,
    from: [i32; 3],
    target: [i32; 3],
    body: bool,
) -> Option<String> {
    let c = crate::compiler::nav::cell_center(from);
    let eye = [c[0], c[1] + EYE_HEIGHT, c[2]];
    let t = crate::compiler::nav::cell_center(target);
    let at = [t[0], t[1] + if body { 1.0 } else { 0.5 }, t[2]];
    let d = ((eye[0] - at[0]).powi(2) + (eye[1] - at[1]).powi(2) + (eye[2] - at[2]).powi(2)).sqrt();
    if d > SIGHT_BLOCKS {
        return Some(format!(
            "{d:.1} blocks away, past the {SIGHT_BLOCKS} blocks sight reaches"
        ));
    }
    let floor = |p: [f64; 3]| [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
    let skip = [from, floor(eye), target, floor(at)];
    crate::compiler::nav::walk_cells(eye, at, |cell| {
        !skip.contains(&cell) && world.blocks_camera(cell)
    })
    .map(|cell| format!("{d:.1} blocks away and hidden: the line is stopped first at {cell:?}"))
}

/// **An act the party can find** (`DW0863`, spec-0104 §3). On every walked
/// path, every act ([`crate::compiler::acts`]) is announced with a `hint` (an
/// objective only), or in sight from the cell the party stands on before it —
/// a clear line by [`crate::compiler::nav::walk_cells`] over
/// [`crate::compiler::nav::World::blocks_camera`], within [`SIGHT_BLOCKS`] —
/// and, for a `kill`, its wave's anchor within the wave's reach of that cell
/// too. A kill fired by acts is judged from each firing act's cell, all of
/// which must see it; a kill on no walk is judged from those cells alone.
///
/// Every refusal is returned; the build reports the first as its verdict and
/// the rest as further errors.
pub fn check_acts_found(
    plan: &crate::compiler::plan::Plan,
    world: &crate::compiler::nav::World,
) -> (ActBinding, Vec<crate::compiler::failure::Failure>) {
    judge_acts(plan, world, crate::compiler::acts::walks(plan))
}

/// [`check_acts_found`] over the walks it is handed — the plan's own, or a
/// test's — and, for every `kill` objective on none of them, the acts that
/// fire its wave.
pub fn judge_acts(
    plan: &crate::compiler::plan::Plan,
    world: &crate::compiler::nav::World,
    walks: &crate::compiler::acts::ActWalks,
) -> (ActBinding, Vec<crate::compiler::failure::Failure>) {
    use crate::compiler::acts::ActWhat;
    let c = plan.campaign;
    let guidance = &c.quests.content.guidance;
    let mut b = ActBinding::default();
    let mut refusals: Vec<crate::compiler::failure::Failure> = Vec::new();
    let mut refused_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    b.walks = walks.walks.len();
    let mut on_a_walk: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let objective = |qi: usize, oi: usize| &c.quests.content.quests[qi].objectives[oi];
    for w in &walks.walks {
        // Each act is judged in the world its walk stands in when the act is
        // next: the doors the walk's earlier steps open are open, the ones a
        // later beat opens are shut.
        let steps: Vec<usize> = w.acts.iter().map(|a| a.step).collect();
        let (worlds, at) = match &w.gates {
            None => {
                let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
                crate::compiler::nav::worlds_at(world, &plan.region_events, &ancestor, &steps)
            }
            Some(gates) => {
                let (events, anc) = &**gates;
                let ancestor =
                    |g: usize, s: usize| g == 0 || anc.get(&s).is_some_and(|x| x.contains(&g));
                crate::compiler::nav::worlds_at(world, events, &ancestor, &steps)
            }
        };
        for (ai, a) in w.acts.iter().enumerate() {
            let world: &crate::compiler::nav::World = worlds[at[ai]].as_ref().unwrap_or(world);
            b.acts += 1;
            on_a_walk.insert(a.id.as_str());
            let obj = match a.what {
                ActWhat::Objective { quest, objective: oi } => Some(objective(quest, oi)),
                ActWhat::Trigger(_) => None,
            };
            if obj.is_none() {
                b.triggers += 1;
            }
            if let Some(o) = obj
                && o.announced(guidance)
                && o.hint().map(str::trim).is_some_and(|h| !h.is_empty())
            {
                b.announced += 1;
                continue;
            }
            // The cells the party judges from: for a kill, each act that fires
            // its wave (every one placed, as the reach rule always required);
            // for every other act, the cell the party stands on before it.
            let mut kill: Option<(f64, &'static str)> = None;
            let mut vantages: Vec<([i32; 3], String)> = Vec::new();
            let mut why: Vec<String> = Vec::new();
            if let Some(Objective::Kill { wave, .. }) = obj {
                let w = c
                    .quests
                    .content
                    .waves
                    .iter()
                    .find(|x| x.id.as_str() == wave.as_str());
                if let Some(wv) = w {
                    kill = Some(crate::compiler::nav::wave_aggro_radius(wv));
                }
                let sites = firing_sites(plan, wave.as_str());
                if sites.is_empty() {
                    why.push(format!("nothing in the campaign fires `spawn-wave` for `{wave}`"));
                }
                for s in &sites {
                    match &s.places {
                        Some(ps) => {
                            for p in ps {
                                vantages
                                    .push((*p, format!("the act at `{}` that fires it", s.path)));
                            }
                        }
                        None => why.push(format!(
                            "`{}` fires its wave from a root that places the party nowhere this \
                             rule can name",
                            s.path
                        )),
                    }
                }
            } else {
                match a.from {
                    Some(f) => vantages.push((f, a.from_what.clone())),
                    None => continue, // no entry cell: the plan's own refusal
                }
            }
            for (v, what) in &vantages {
                let reasons = act_unfound(world, *v, a.target, a.body, kill);
                // A fight's refusal says whether its muster is in sight, so the
                // author knows which of the two halves failed.
                if kill.is_some()
                    && !reasons.is_empty()
                    && unseen(world, *v, a.target, a.body).is_none()
                {
                    why.push(format!(
                        "from {what} ({v:?}) the wave's anchor {:?} is in sight",
                        a.target
                    ));
                }
                for r in reasons {
                    why.push(format!("from {what} ({v:?}) {r}"));
                }
            }
            if why.is_empty() {
                b.in_sight += 1;
                if kill.is_some() {
                    b.kills_in_reach += 1;
                }
                continue;
            }
            b.refused += 1;
            if !refused_ids.insert(a.id.clone()) {
                continue;
            }
            let remedy = match obj {
                Some(o) => {
                    let how = if o.title().is_none() {
                        "it has no `title`"
                    } else if !o.announced(guidance) {
                        "its announcement is hidden"
                    } else {
                        "it has no `hint`"
                    };
                    format!(
                        "It is not announced with a hint ({how}). Give `{}` a `title` and a \
                         `hint` that says where it is, or move the act or its target into \
                         sight of where the party stands{}.",
                        a.id,
                        if kill.is_some() {
                            " and the wave within its reach. Do NOT widen `follow_range` to buy \
                             the distance: that retunes the fight to pass a proof"
                        } else {
                            ""
                        }
                    )
                }
                None => format!(
                    "A trigger has no lines to say where it is, so its only remedy is the \
                     world: move `{}` or what it acts on into sight of where the party stands, \
                     or make the act an `interact` objective with a `hint`.",
                    a.id
                ),
            };
            refusals.push(crate::compiler::failure::Failure {
                code: DW_FIGHT_UNSIGNED,
                message: format!(
                    "on {}, the `{}` act `{}` cannot be found by the party: {}. An act the \
                     party can see from where it stands needs no line; one it cannot see owes \
                     one. {remedy}",
                    w.label,
                    a.kind,
                    a.id,
                    why.join("; "),
                ),
            });
        }
    }
    // A kill on no walk: judged from the acts that fire its wave, as before
    // this rule reached every act — so widening it to every act on a walk
    // loosens nothing it already judged.
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            let Objective::Kill { id, wave, .. } = o else {
                continue;
            };
            if on_a_walk.contains(id.as_str()) {
                continue;
            }
            b.kills_off_walk += 1;
            if o.announced(guidance) && o.hint().map(str::trim).is_some_and(|h| !h.is_empty()) {
                continue;
            }
            let Some(wv) = c
                .quests
                .content
                .waves
                .iter()
                .find(|w| w.id.as_str() == wave.as_str())
            else {
                continue; // an unknown wave is `DW0171`'s refusal
            };
            let (r, src) = crate::compiler::nav::wave_aggro_radius(wv);
            let anchor = crate::compiler::plan::wave_area(c, wave.as_str())
                .and_then(|a| plan.point(a, wv.anchor.as_str()));
            let sites = firing_sites(plan, wave.as_str());
            let mut why: Vec<String> = Vec::new();
            if sites.is_empty() {
                why.push(format!("nothing in the campaign fires `spawn-wave` for `{wave}`"));
            }
            match anchor {
                None => why.push(format!(
                    "`{wave}`'s anchor `{}` resolves to no cell",
                    wv.anchor.as_str()
                )),
                Some(at) => {
                    for s in &sites {
                        match &s.places {
                            None => why.push(format!(
                                "`{}` fires it from a root that places the party nowhere this \
                                 rule can name",
                                s.path
                            )),
                            Some(ps) => {
                                for p in ps {
                                    if let Some(u) = unseen(world, *p, at, true) {
                                        why.push(format!(
                                            "from `{}` ({p:?}) the wave's anchor is {u}",
                                            s.path
                                        ));
                                    }
                                    let d = dist(*p, at);
                                    if d > r {
                                        why.push(format!(
                                            "`{}` fires it from {p:?}, {d:.1} blocks from the \
                                             anchor, past its reach of {r} blocks ({src})",
                                            s.path
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if why.is_empty() {
                continue;
            }
            b.refused += 1;
            refusals.push(crate::compiler::failure::Failure {
                code: DW_FIGHT_UNSIGNED,
                message: format!(
                    "`kill` objective `{id}` (quest {qi}, objective {oi}) stands on no walk and \
                     the party has no way to find wave `{wave}`: it is not announced with a \
                     `hint`, and {}. Give it a `title` and a `hint` saying where the wave \
                     arrives, or have every act that fires the wave stand in sight of its \
                     anchor and within its reach. Do NOT widen `follow_range` to buy the \
                     distance: that retunes the fight to pass a proof.",
                    why.join("; ")
                ),
            });
        }
    }
    (b, refusals)
}

/// Euclidean distance between two cells' centres.
fn dist(a: [i32; 3], b: [i32; 3]) -> f64 {
    let dx = f64::from(a[0] - b[0]);
    let dy = f64::from(a[1] - b[1]);
    let dz = f64::from(a[2] - b[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Every effect root whose bundle fires `spawn-wave` for `wave_id`, deep, with
/// the place the party stands at when it fires.
fn firing_sites(plan: &crate::compiler::plan::Plan, wave_id: &str) -> Vec<FiringSite> {
    let c = plan.campaign;
    let mut out = Vec::new();
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        let mut fires = false;
        for e in list {
            e.visit_deep(&mut |x| {
                if matches!(x.spawn_wave(), Some(w) if w.as_str() == wave_id) {
                    fires = true;
                }
            });
        }
        if fires {
            out.push(FiringSite {
                path: site.path.clone(),
                places: act_places(plan, &site.owner),
            });
        }
    });
    out
}

/// **Where the party stands when a root's bundle fires** (spec-0093 §5): the
/// placed acts, per root kind, and `None` for the roots that put the party
/// nowhere this rule can name.
fn act_places(
    plan: &crate::compiler::plan::Plan,
    owner: &delvewright_dsl::EffectRootOwner<'_>,
) -> Option<Vec<[i32; 3]>> {
    use delvewright_dsl::{EffectRootOwner, Fight, TriggerOn};
    let c = plan.campaign;
    match owner {
        EffectRootOwner::ObjectiveComplete { quest, objective } => {
            let o = quest
                .objectives
                .iter()
                .find(|o| o.id().as_str() == *objective)?;
            objective_place(plan, quest, o).map(|p| vec![p])
        }
        // A quest completes when its last objective does: every objective no
        // other objective of the quest declares `after` on. All of them must
        // place the party, because any of them can be the one that completes it.
        EffectRootOwner::QuestComplete { quest } => {
            let sinks: Vec<&Objective> = quest
                .objectives
                .iter()
                .filter(|o| {
                    !quest
                        .objectives
                        .iter()
                        .any(|x| x.after().iter().any(|a| a.as_str() == o.id().as_str()))
                })
                .collect();
            if sinks.is_empty() {
                return None;
            }
            sinks
                .iter()
                .map(|o| objective_place(plan, quest, o))
                .collect::<Option<Vec<_>>>()
        }
        EffectRootOwner::Trigger(t) => match &t.on {
            TriggerOn::Strike | TriggerOn::Use | TriggerOn::Approach { .. } | TriggerOn::Step => t
                .at_anchor()
                .and_then(|a| plan.point_any(a))
                .map(|p| vec![p]),
            TriggerOn::StrikeNpc { npc } => c
                .npcs
                .content
                .npcs
                .iter()
                .find(|n| n.id.as_str() == npc.as_str())
                .and_then(|n| plan.body_point(delvewright_dsl::BodyRef::Npc(n)))
                .map(|p| vec![p]),
            TriggerOn::StrikeAssembly { assembly } => {
                assembly_place(plan, assembly.as_str()).map(|p| vec![p])
            }
        },
        EffectRootOwner::TrapPayload(t) => plan
            .traps
            .iter()
            .find(|tp| tp.id == t.id.as_str())
            .map(|tp| tp.trigger_cell)
            .or_else(|| plan.point_any(t.at.as_str()))
            .map(|p| vec![p]),
        EffectRootOwner::ShopOffer(h) => plan.point_any(h.anchor.as_str()).map(|p| vec![p]),
        EffectRootOwner::OnKill(Fight::Wave(w)) => wave_place(plan, w.id.as_str()).map(|p| vec![p]),
        EffectRootOwner::OnKill(Fight::Actor(a)) => plan
            .body_point(delvewright_dsl::BodyRef::Actor(a))
            .map(|p| vec![p]),
        EffectRootOwner::AssemblyLand(m) => assembly_place(plan, m.id.as_str()).map(|p| vec![p]),
        EffectRootOwner::DialogueRespawn
        | EffectRootOwner::ShortcutUnlock(_)
        | EffectRootOwner::OnDeath
        | EffectRootOwner::LoopCross(_) => None,
    }
}

/// Where an objective completes: the critical path's step for it when the path
/// performs it, else its own anchor resolved in its quest's area, else the body
/// or wave it is about.
fn objective_place(
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
            container,
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
        Verb::Narrate { text, .. } => {
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
// DW1015 — a title names the act its type completes on (spec-0104 §4)
// ---------------------------------------------------------------------------

delvewright_dsl::dw_code! {
    /// `DW1015`: an objective's **title** carries no verb of its own kind's set
    /// in the act lexicon (`crates/dsl/data/act-verbs.json`) — it names another
    /// kind's act, or no act — or its **hint** names another kind's act and none
    /// of its own (spec-0104 §4).
    ///
    /// The objective's `type` is the engine's record of what the hands must do;
    /// the verb a title opens with is the author's claim about it. Two authored
    /// fields that disagree are judged as a contradiction between declarations,
    /// by membership in a closed set, never by reading the prose for meaning.
    /// A hint with no lexicon verb passes: a hint may be a place.
    pub const DW_ACT_MISNAMED: DwCode = DwCode::new("DW1015", ExitTier::Build);
}

/// What [`check_act_verbs`] judged, stated whichever way it went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActVerbBinding {
    /// Titles judged, over every judged language.
    pub titles: usize,
    /// Hints judged, over every judged language.
    pub hints: usize,
    /// Languages judged (the English document and each sidecar with a table).
    pub languages: usize,
    /// Objective strings in sidecar languages with no table — unjudged.
    pub unjudged: usize,
    /// Strings refused.
    pub refused: usize,
}

impl ActVerbBinding {
    /// The one line a run prints about this rule.
    pub fn line(&self) -> String {
        format!(
            "DW1015 binding: {} title(s) and {} hint(s) over {} language(s) judged ({} string(s) \
             in languages with no table); {} refused",
            self.titles, self.hints, self.languages, self.unjudged, self.refused
        )
    }
}

/// **A title names the act its type completes on** (`DW1015`), over the English
/// document and every sidecar whose language has a table in the act lexicon.
pub fn check_act_verbs(
    c: &Campaign,
    sidecars: &std::collections::BTreeMap<String, delvewright_dsl::l10n::L10nDoc>,
) -> (Vec<Diagnostic>, ActVerbBinding) {
    use delvewright_dsl::actverbs;
    use delvewright_dsl::l10n::local_id;
    let mut d = Vec::new();
    let mut b = ActVerbBinding::default();
    // (language, its table, a lookup of each objective string's text there)
    let mut langs: Vec<(String, &actverbs::Table, Option<&delvewright_dsl::l10n::L10nDoc>)> =
        Vec::new();
    if let Some(t) = actverbs::table("en") {
        langs.push(("en".to_string(), t, None));
    }
    for (lang, doc) in sidecars {
        match actverbs::table(lang) {
            Some(t) => langs.push((lang.clone(), t, Some(doc))),
            None => {
                b.unjudged += doc
                    .content
                    .keys()
                    .filter(|k| {
                        k.starts_with("obj.") && (k.ends_with(".title") || k.ends_with(".hint"))
                    })
                    .count();
            }
        }
    }
    b.languages = langs.len();
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        let ql = local_id(q.id.as_str());
        for (oi, o) in q.objectives.iter().enumerate() {
            let ol = local_id(o.id().as_str());
            let kind = o.kind();
            let id = o.id().as_str();
            for (lang, table, doc) in &langs {
                let text_of = |field: &str, english: Option<&str>| -> Option<String> {
                    match doc {
                        None => english.map(str::to_string),
                        Some(doc) => doc
                            .content
                            .get(&format!("obj.{ql}.{ol}.{field}"))
                            .cloned(),
                    }
                };
                let at = |field: &str| match doc {
                    None => ("quests", format!("/content/quests/{qi}/objectives/{oi}/{field}")),
                    Some(_) => (
                        "l10n",
                        format!("l10n/{lang}.json#/content/obj.{ql}.{ol}.{field}"),
                    ),
                };
                let own = table.listed(kind);
                if let Some(title) = text_of("title", o.title()) {
                    b.titles += 1;
                    let shown = delvewright_dsl::l10n::plain(&title).into_owned();
                    let found = table.verbs_in(&shown);
                    if !found.iter().any(|f| f.kind == kind) {
                        b.refused += 1;
                        let what = match found.first() {
                            Some(f) => format!(
                                "names another act: `{}` is a `{}` verb",
                                f.verb, f.kind
                            ),
                            None => "names no act: it carries no verb of the lexicon".to_string(),
                        };
                        let (stage, path) = at("title");
                        d.push(Diagnostic::error(
                            DW_ACT_MISNAMED,
                            stage,
                            path,
                            format!(
                                "the {lang} title of `{kind}` objective `{id}` (`{shown}`) {what}. \
                                 A title is the party's instruction, and its verb is a claim \
                                 about what the hands will do; a `{kind}` completes on its own \
                                 act, so its title carries one of the `{kind}` verbs: {own}. \
                                 Rewrite the title with one of them (the rest of the title is \
                                 yours: `Go up the stair and ring the bell` is an `interact`). A \
                                 verb this kind needs that is not listed is an engine edit to \
                                 `crates/dsl/data/act-verbs.json`, not a reason to keep the \
                                 title."
                            ),
                        ));
                    }
                }
                if let Some(hint) = text_of("hint", o.hint()) {
                    b.hints += 1;
                    let shown = delvewright_dsl::l10n::plain(&hint).into_owned();
                    let found = table.verbs_in(&shown);
                    let other = found.iter().find(|f| f.kind != kind);
                    if let Some(f) = other
                        && !found.iter().any(|f| f.kind == kind)
                    {
                        b.refused += 1;
                        let (stage, path) = at("hint");
                        d.push(Diagnostic::error(
                            DW_ACT_MISNAMED,
                            stage,
                            path,
                            format!(
                                "the {lang} hint of `{kind}` objective `{id}` (`{shown}`) names \
                                 another act: `{}` is a `{}` verb, and the hint carries none of \
                                 the `{kind}` verbs ({own}). A hint that tells the party to do \
                                 something the objective does not complete on sends them to do \
                                 the wrong thing. Name the act this objective completes on, or \
                                 write the hint as a place (`Past the bell, left of the arch.`), \
                                 which passes.",
                                f.verb, f.kind
                            ),
                        ));
                    }
                }
            }
        }
    }
    (d, b)
}

// ---------------------------------------------------------------------------
// DW1016 — the opening teaches each input (spec-0104 §5.5)
// ---------------------------------------------------------------------------

delvewright_dsl::dw_code! {
    /// `DW1016`: the campaign's `guidance.teaching` and what its walked paths
    /// need disagree (spec-0104 §5.5) — **A**: an input class some walked path
    /// requires that `teaching` neither gives a line nor declares `null`; **B**:
    /// a class `teaching` gives a line and no walked path requires; **C**: a
    /// line without its class's key span (`[[key|use]]` for the two `use-`
    /// classes, `[[key|attack]]` for the two `attack-` classes).
    ///
    /// The Stranding round 1: nothing in the opening said that the world's
    /// blocks are right-clicked, and a sign started a cutscene unannounced. The
    /// class of each act is derived from its kind and body, never typed; what
    /// the author declares is how the level teaches each class.
    pub const DW_INPUT_UNTAUGHT: DwCode = DwCode::new("DW1016", ExitTier::Build);
}

/// What [`check_teaching`] examined, stated whichever way it went.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TeachingBinding {
    /// Input classes some walked path requires.
    pub required: usize,
    /// Walks taken.
    pub walks: usize,
    /// Of the classes declared, taught by a line.
    pub lines: usize,
    /// Of the classes declared, `null` — taught by the level.
    pub level: usize,
    /// Refusals.
    pub refused: usize,
}

impl TeachingBinding {
    /// The one line a build prints about this rule.
    pub fn line(&self) -> String {
        format!(
            "DW1016 binding: {} class(es) required over {} walk(s) — {} taught by a line, {} \
             declared taught by the level; {} refused",
            self.required, self.walks, self.lines, self.level, self.refused
        )
    }
}

/// Whether `line` carries a key span naming `input`.
fn says_the_key(line: &str, input: &str) -> bool {
    delvewright_dsl::textstyle::parse(line)
        .map(|segs| {
            segs.iter().any(|g| {
                matches!(g, delvewright_dsl::textstyle::Segment::Span(st, _) if st.key == Some(input))
            })
        })
        .unwrap_or(false)
}

/// **The opening teaches each input** (`DW1016`): every input class a walked
/// path requires is named by `guidance.teaching` (a line, or `null`); every
/// line is owed by some walk; every line says its class's key.
pub fn check_teaching(
    plan: &crate::compiler::plan::Plan,
) -> (TeachingBinding, Vec<crate::compiler::failure::Failure>) {
    use delvewright_dsl::{InputClass, TeachLine};
    let teaching = &plan.campaign.quests.content.guidance.teaching;
    let required = crate::compiler::acts::required(plan);
    let mut b = TeachingBinding {
        required: required.len(),
        walks: crate::compiler::acts::walks(plan).walks.len(),
        ..TeachingBinding::default()
    };
    let mut out = Vec::new();
    for class in InputClass::ALL {
        let name = class.as_str();
        let input = class.input();
        let entry = teaching.get(class);
        match entry {
            Some(TeachLine::Line(_)) => b.lines += 1,
            Some(TeachLine::Level) => b.level += 1,
            None => {}
        }
        match (required.get(&class), entry) {
            (Some((walk, act)), None) => out.push(crate::compiler::failure::Failure {
                code: DW_INPUT_UNTAUGHT,
                message: format!(
                    "input class `{name}` is needed and never taught: on {walk}, `{act}` is the \
                     first act that asks the party to {}, and `guidance.teaching` neither gives \
                     `{name}` a line nor declares it `null`. A player who has never done this \
                     in this world does not know to. Write `guidance.teaching.{name}`: a line \
                     in the campaign's voice carrying `[[key|{input}]]`, shown once per player \
                     the first time such an act is next — or `null`, if the level itself \
                     teaches it (a scene the party watches, a first act that cannot be missed).",
                    what_the_hands_do(class)
                ),
            }),
            (None, Some(TeachLine::Line(_))) => out.push(crate::compiler::failure::Failure {
                code: DW_INPUT_UNTAUGHT,
                message: format!(
                    "`guidance.teaching.{name}` gives a line and no walked path requires the \
                     class: no act on any walk asks the party to {}, so the line is shown to \
                     nobody. Remove the line.",
                    what_the_hands_do(class)
                ),
            }),
            _ => {}
        }
        if let Some(TeachLine::Line(line)) = entry
            && !says_the_key(line, input)
        {
            out.push(crate::compiler::failure::Failure {
                code: DW_INPUT_UNTAUGHT,
                message: format!(
                    "`guidance.teaching.{name}` does not say the key: the line `{line}` carries \
                     no `[[key|{input}]]`. The key span is the one thing only the engine knows — \
                     the client draws the viewer's own bound key there — and a teaching line \
                     that does not carry it teaches the act without the input. Write \
                     `[[key|{input}]]` into the line where the key is named."
                ),
            });
        }
    }
    b.refused = out.len();
    (b, out)
}

/// What the hands do for a class, as a clause.
fn what_the_hands_do(class: delvewright_dsl::InputClass) -> &'static str {
    use delvewright_dsl::InputClass;
    match class {
        InputClass::UseBlock => "right-click a block",
        InputClass::UseBody => "right-click a person or a body",
        InputClass::AttackBody => "left-click a body",
        InputClass::AttackBlock => "left-click a block",
    }
}

// ---------------------------------------------------------------------------
// DW1017 — a pressable stands alone among its kind (spec-0104 §6)
// ---------------------------------------------------------------------------

delvewright_dsl::dw_code! {
    /// `DW1017`: a **pressable** — an `interact`'s prop or authored block, a
    /// `use`/`strike` trigger's prop or authored block, a `collect`'s adopted
    /// container — has a block of the same id (blockstate ignored) within
    /// [`crate::compiler::crosshair::INTERACTION_REACH`] of the pressed cell,
    /// and the act is not a prop-less `interact` whose marker is shown
    /// (spec-0104 §6).
    ///
    /// The Stranding round 1: one prop stood among many identical blocks with
    /// nothing to say which to press. Whether a block reads as the thing its
    /// text names is a judgement; that nothing of its own kind stands within the
    /// click's reach of it is the machine's half.
    pub const DW_PRESSABLE_AMONG_TWINS: DwCode = DwCode::new("DW1017", ExitTier::Build);
}

/// What [`check_pressables_alone`] examined, stated whichever way it went.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TwinBinding {
    /// Pressables examined.
    pub pressables: usize,
    /// Of those, alone within the click's reach.
    pub alone: usize,
    /// Of those, prop-less `interact`s whose marker is shown — the lantern
    /// picks them out of their twins.
    pub marked: usize,
    /// Of those, refused.
    pub refused: usize,
    /// Same-kind cells found within reach of a refused pressable.
    pub twin_cells: usize,
}

impl TwinBinding {
    /// The one line a build prints about this rule.
    pub fn line(&self) -> String {
        format!(
            "DW1017 binding: {} pressable(s) — {} alone within {:.1} blocks, {} marked; {} \
             refused among {} twin cell(s)",
            self.pressables,
            self.alone,
            crate::compiler::crosshair::INTERACTION_REACH,
            self.marked,
            self.refused,
            self.twin_cells
        )
    }
}

/// A block id without its namespace and state: `minecraft:lever[facing=north]`
/// and `lever` are one kind.
fn kind_of(block: &str) -> &str {
    let id = crate::compiler::pressable::block_id(block);
    id.strip_prefix("minecraft:").unwrap_or(id)
}

/// **A pressable stands alone among its kind** (`DW1017`), over the settled
/// block map with every pressable's own prop placed at its cell.
pub fn check_pressables_alone(
    plan: &crate::compiler::plan::Plan,
    blocks: &crate::compiler::blockstate::BlockMap,
) -> (TwinBinding, Vec<crate::compiler::failure::Failure>) {
    use crate::compiler::pressable::Body;
    use delvewright_dsl::TriggerOn;
    let c = plan.campaign;
    let guidance = &c.quests.content.guidance;
    let mut b = TwinBinding::default();
    // The block in a cell: a prop placed there, else the settled map.
    let mut props: std::collections::BTreeMap<[i32; 3], String> =
        std::collections::BTreeMap::new();
    // (what, the pressed cell, its block)
    let mut pressables: Vec<(String, [i32; 3], String)> = Vec::new();
    let authored = |pos: [i32; 3]| -> Option<([i32; 3], String)> {
        [pos, [pos[0], pos[1] + 1, pos[2]]].into_iter().find_map(|cell| {
            blocks
                .get(&cell)
                .filter(|name| !delvewright_dsl::blockshape::is_air(name.as_str()))
                .map(|name| (cell, name.as_str().to_string()))
        })
    };
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            match o {
                Objective::Interact {
                    id, anchor, prop, ..
                } => {
                    let area = plan.quest_area(q.id.as_str());
                    let Some(pos) = area
                        .and_then(|a| plan.point(a, anchor.as_str()))
                        .or_else(|| plan.point_any(anchor.as_str()))
                    else {
                        continue;
                    };
                    let what = format!("`interact` objective `{}`", id.as_str());
                    if let Some(p) = prop {
                        props.insert(pos, p.block.clone());
                        pressables.push((what, pos, p.block.clone()));
                    } else if o.marker_shown(guidance) {
                        b.pressables += 1;
                        b.marked += 1;
                    } else if let Some((cell, block)) = authored(pos) {
                        pressables.push((what, cell, block));
                    }
                }
                Objective::Collect {
                    id,
                    container: Some(_),
                    ..
                } => {
                    if let Some(f) = plan
                        .collect_fills
                        .iter()
                        .find(|f| f.objective_id == id.as_str())
                        && let Some(block) = blocks.get(&f.cell)
                    {
                        pressables.push((
                            format!("`collect` objective `{}`'s container", id.as_str()),
                            f.cell,
                            block.as_str().to_string(),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    for t in &c.quests.content.triggers {
        if !matches!(t.on, TriggerOn::Use | TriggerOn::Strike) {
            continue;
        }
        let what = format!("`{}` trigger `{}`", t.on.kind(), t.id.as_str());
        match crate::compiler::pressable::trigger_body(plan, t) {
            Body::Block { cell, block } => {
                props.insert(cell, block.clone());
                pressables.push((what, cell, block));
            }
            Body::Point(pos) => {
                if let Some(p) = &t.prop {
                    props.insert(pos, p.block.clone());
                    pressables.push((what, pos, p.block.clone()));
                } else if crate::compiler::emit::npc_ridden_by(plan, t).is_none()
                    && let Some((cell, block)) = authored(pos)
                {
                    pressables.push((what, cell, block));
                }
            }
            Body::Rides { .. } | Body::Region(_) | Body::Nothing => {}
        }
    }
    let reach = crate::compiler::crosshair::INTERACTION_REACH;
    let r = reach.floor() as i32;
    let mut out = Vec::new();
    for (what, cell, block) in &pressables {
        b.pressables += 1;
        let kind = kind_of(block);
        let mut twins: Vec<([i32; 3], f64)> = Vec::new();
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    let d = f64::from(dx * dx + dy * dy + dz * dz).sqrt();
                    if d > reach {
                        continue;
                    }
                    let at = [cell[0] + dx, cell[1] + dy, cell[2] + dz];
                    let there = props
                        .get(&at)
                        .map(String::as_str)
                        .or_else(|| blocks.get(&at).map(|s| s.as_str()));
                    if there.is_some_and(|n| kind_of(n) == kind) {
                        twins.push((at, d));
                    }
                }
            }
        }
        if twins.is_empty() {
            b.alone += 1;
            continue;
        }
        b.refused += 1;
        b.twin_cells += twins.len();
        let listed: Vec<String> = twins
            .iter()
            .map(|(at, d)| format!("{at:?} at {d:.1} blocks"))
            .collect();
        out.push(crate::compiler::failure::Failure {
            code: DW_PRESSABLE_AMONG_TWINS,
            message: format!(
                "{what} is pressed at {cell:?} on a `{kind}`, and {} more `{kind}` stand within \
                 the click's reach ({reach:.1} blocks) of it: {}. Nothing tells the party which \
                 one to press. Move the twins away, change the prop to a block the scene has \
                 one of, or mark the act (a prop-less `interact` with `marker: shown`). Do NOT \
                 move the anchor into a wall to satisfy this: the cell a body presses is the \
                 cell this reads.",
                twins.len(),
                listed.join(", ")
            ),
        });
    }
    (b, out)
}
