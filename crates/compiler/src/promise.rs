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
//!
//! # Why these live together and not next to the verbs they judge
//!
//! Each of the four is about the SAME object class — a player-facing promise
//! attached to an objective — and none of them is about the verb that happens to
//! carry it. Split across `loot.rs`, `nav.rs`, `cast.rs` and `combat.rs` they
//! would be four private readings of "what did the party get told", which is
//! exactly the shape [`crate::cast`] already had to unpick once. One module,
//! one reading of the prompt surface.
//!
//! # What the prompt surface actually is
//!
//! An objective carries two player-facing strings and no others:
//! [`Objective::title`] and [`Objective::hint`]. Both are optional. What the
//! emitter does with them is the load-bearing fact underneath three of these
//! four rules, and it is not obvious from the schema — `emit::emit_datapack`
//! wraps the whole activation announcement in `if v03 && let Some(title)`, and
//! nests the hint's `tellraw` *inside* that. So:
//!
//! * an objective with no `title` announces **nothing at all** — no chat line,
//!   no cue sound, and its wayfinding marker is summoned nameless
//!   (`emit::marker_name_fields` refuses to surface a raw `obj/…` id);
//! * an objective with a `hint` and no `title` shows **neither** — the hint is
//!   authored prose the emitter drops on the floor, with nothing anywhere
//!   saying so.
//!
//! # Every version, deliberately
//!
//! All four judge what the document SAYS — a required field absent, or two
//! authored fields contradicting each other — which is [`Binds::EveryVersion`]'s
//! own stated category. None of them requires a campaign to HAVE anything it
//! could not have had at any version: `title`, `hint` and `item_name` are all
//! v0.3/v0.8 surface, and every campaign that declares an objective could
//! always have written them. Measured before choosing, over every campaign blob
//! reachable from the content repository: see the module tests and the round's
//! blast-radius record.
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

use delvewright_dsl::{Campaign, Diagnostic, DwCode, NarrateStyle, Objective, QuestEffect};

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
pub const DW_CLOCK_UNREAD: DwCode = DwCode::every_version("DW0860");

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
pub const DW_ADOPTED_CONTAINER_UNMARKED: DwCode = DwCode::every_version("DW0861");

/// `DW0862`: an objective authors a `hint` and no `title`, so the emitter shows
/// **neither** and the prompt reaches no player.
///
/// The activation announcement is emitted only for a titled objective, and the
/// hint's `tellraw` is nested inside that guard — so a hint without a title is
/// prose that is inventoried for translation, rendered into every language
/// sidecar, and never once put on a screen. Nothing else in the toolchain says
/// so: it is not a warning, not a lint, and the l10n inventory counts it as a
/// live string.
pub const DW_PROMPT_UNSHOWN: DwCode = DwCode::every_version("DW0862");

/// `DW0863`: a `kill` objective with no `title`, or with no `hint`.
///
/// Bell round 6: a defence wave gave the party no guidance to where the attackers
/// were, so the fight could not be found. A fight is the one objective kind the
/// compiler gives the world **nothing** for. Measured against the emitter rather
/// than assumed: `emit::activation_commands` returns an empty command list for
/// `Objective::Kill`, `emit::completion_cleanup` likewise, and the render plan
/// falls back to the literal phrase `the fight` because no name exists to use.
/// Every other kind leaves something standing — a `reach-anchor` a glowing end
/// rod, an `interact` a lantern or its authored prop, a `collect` a chest, a
/// `talk-to` a named body. A wave is bodies that appear somewhere, and the
/// objective's own two lines are the only thing that can say where.
pub const DW_FIGHT_UNSIGNED: DwCode = DwCode::every_version("DW0863");

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
    /// Of those, `kill` objectives — [`DW_FIGHT_UNSIGNED`]'s population.
    pub kill_objectives: usize,
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
            "promise: {} objective(s) examined — {} `kill` (DW0863), {} adopted container(s) \
             (DW0861); {} failure clock(s) (DW0860) over {} effect root(s)",
            self.objectives,
            self.kill_objectives,
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
    check_failure_clocks(c, &mut d, &mut b);
    (d, b)
}

// ---------------------------------------------------------------------------
// The three objective-prompt rules
// ---------------------------------------------------------------------------

/// `DW0861`, `DW0862`, `DW0863` — one walk of the objective surface, because all
/// three ask about the same two strings.
fn check_objective_prompts(c: &Campaign, d: &mut Vec<Diagnostic>, b: &mut PromiseBinding) {
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            b.objectives += 1;
            let path = format!("/content/quests/{qi}/objectives/{oi}");
            let id = o.id().as_str();
            let title = o.title().map(str::trim).filter(|s| !s.is_empty());
            let hint = o.hint().map(str::trim).filter(|s| !s.is_empty());

            // DW0862 — a hint the emitter will never show.
            if hint.is_some() && title.is_none() {
                d.push(Diagnostic::error(
                    DW_PROMPT_UNSHOWN,
                    "quests",
                    format!("{path}/hint"),
                    format!(
                        "objective `{id}` authors a `hint` and no `title`, so the party is shown \
                         neither: the activation announcement is emitted only for a titled \
                         objective and the hint's line is nested inside it, so this prose is \
                         inventoried for translation, rendered into every language sidecar, and \
                         never put on a screen. Give `{id}` a `title` — the hint is the second \
                         line of an announcement, not an announcement. Do NOT delete the hint to \
                         clear this: the defect is the missing title, and deleting the hint \
                         silences the beat instead of fixing it."
                    ),
                ));
            }

            // DW0863 — a fight with nothing pointing at it.
            if let Objective::Kill { wave, .. } = o {
                b.kill_objectives += 1;
                let missing = match (title.is_none(), hint.is_none()) {
                    (true, true) => Some("neither a `title` nor a `hint`"),
                    (true, false) => Some("no `title`"),
                    (false, true) => Some("no `hint`"),
                    (false, false) => None,
                };
                if let Some(missing) = missing {
                    d.push(Diagnostic::error(
                        DW_FIGHT_UNSIGNED,
                        "quests",
                        path.clone(),
                        format!(
                            "`kill` objective `{id}` requires the party to fight wave `{wave}` and \
                             carries {missing}. A fight is the one objective kind that leaves \
                             nothing in the world to find: no marker is summoned, no prop is \
                             placed, no name is written, and the party is told to kill something \
                             without being told where it is. Give `{id}` both a `title` and a \
                             `hint` saying where the wave arrives. Do NOT rely on the wave's \
                             anchor being near the previous beat — nothing proves that, and \
                             nothing shows the party an anchor.",
                        ),
                    ));
                }
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
            match eff {
                QuestEffect::Sequence { steps } => {
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
    match eff {
        // Every narrate channel is a prompt: chat, title, subtitle, actionbar and
        // art all put authored words in front of the party. The channel changes
        // where the words sit, never whether they have to be read.
        QuestEffect::Narrate { text, style, .. } => {
            let _: Option<&NarrateStyle> = style.as_ref();
            prompts.push(Prompt {
                at,
                ord: here,
                text: text.clone(),
            });
        }
        QuestEffect::BeginStealth {
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
