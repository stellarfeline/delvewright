//! **A step's clue is presented before the step** (`DW0863`, spec-0143).
//!
//! The one module that answers "has the party been shown what this step's
//! answer rests on". An act — an objective, or an environment trigger — states
//! its clues ([`delvewright_dsl::Clue`]); this module confirms that on the
//! **main line** every stated clue is presented before the party reaches the
//! act, on every walked path that performs it. It judges nothing else: not
//! whether the act can be found, not whether the clue is enough. An act that
//! states no clue is owed nothing, and so is an act the main line never
//! performs — an optional puzzle, where going to look for the clue is play.
//!
//! # The main line
//!
//! The main line is derived, never declared here: the quests of the stage-4
//! **spine** ([`delvewright_dsl::QuestPlanContent::spine`], the one authority
//! on which quests a body cannot reach the finale without). An objective is
//! on the main line when its quest is; a trigger is when the objective the
//! path performs it in front of is. The walks are
//! [`crate::compiler::acts`]'s: the exported critical path and every realised
//! branch's own path, which carries the optional strands its world completes.
//!
//! # Presented, and before
//!
//! *Before* is never the order one walk happens to take. An objective is
//! **forced before** an act when no play order reaches the act without it:
//! the act's strict ancestors (`after`, and every objective of a quest its
//! quest depends on — [`crate::compiler::plan::Plan::strict_ancestor_steps`]),
//! and the one objective whose completion alone sets a flag the act, or
//! anything forced before it, requires.
//!
//! * a `record` clue is presented when its objective is forced before the act:
//!   the finished page holds the line;
//! * a `hint` clue is presented when its objective is the act itself or is
//!   forced before it, or is a main-line objective the replay holds started
//!   by the time the walk reaches the act: the open page holds the line;
//! * a `sight` clue is presented when its anchor's cell is in sight — the one
//!   sight rule, [`crate::compiler::framing`]'s `in_sight`, within
//!   [`SIGHT_BLOCKS`] — from a cell the party must stand on before the act:
//!   the walk's entry cell, the place of a step forced before the act, the
//!   act's own place, or a cell of the route into one of those.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{Campaign, Clue, Diagnostic, DwCode, ExitTier, Objective};

use crate::compiler::acts::{ActWalk, ActWalks};
use crate::compiler::failure::Failure;
use crate::compiler::nav::World;
use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW0863`: a clue a main-line act states is not presented before the
    /// party reaches the act (spec-0143 §4) — its text is on no journal page
    /// that every run reaching the act has opened, or its place is in sight
    /// from no cell the main line stands on before the act. Judged at the
    /// build, where the walks have places and the world has blocks.
    pub const DW_CLUE_UNPRESENTED: DwCode = DwCode::new("DW0863", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1136`: a clue names nothing that could present it (spec-0143 §3) —
    /// an objective that does not exist, a `hint` clue on an objective with no
    /// `hint` (or no `title` to carry it), a `record` clue on an objective with
    /// no `record`, or a `sight` clue whose anchor resolves to no cell.
    pub const DW_CLUE_NAMES_NOTHING: DwCode = DwCode::new("DW1136", ExitTier::Build);
}

/// The one reach sight is given (spec-0104 §2.6): the farthest the pinned
/// client draws a body of a block or less at the default *Entity Distance*
/// (`64 × size × viewScale`, size clamped to at least 1), used for blocks as
/// well.
pub const SIGHT_BLOCKS: f64 = 64.0;

/// The eye of a body standing on a cell: [`crate::compiler::nav::cell_center`]
/// plus the standing eye height.
const EYE_HEIGHT: f64 = 1.62;

// ---------------------------------------------------------------------------
// What is stated — the document's half (`DW1136`)
// ---------------------------------------------------------------------------

/// One act that states a clue, as the documents declare it.
struct Stated<'c> {
    /// The objective's or trigger's id.
    id: String,
    /// `objective` or `trigger`, for a message.
    what: &'static str,
    /// The JSON pointer of its `clues` list in `quests`.
    path: String,
    /// The clues, in declared order.
    clues: &'c [Clue],
}

/// Every act that states a clue, objectives first, in document order — the one
/// enumeration both tiers read.
fn stated(c: &Campaign) -> Vec<Stated<'_>> {
    let mut out = Vec::new();
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            if !o.clues().is_empty() {
                out.push(Stated {
                    id: o.id().as_str().to_string(),
                    what: "objective",
                    path: format!("/content/quests/{qi}/objectives/{oi}/clues"),
                    clues: o.clues(),
                });
            }
        }
    }
    for (ti, t) in c.quests.content.triggers.iter().enumerate() {
        if !t.clues.is_empty() {
            out.push(Stated {
                id: t.id.as_str().to_string(),
                what: "trigger",
                path: format!("/content/triggers/{ti}/clues"),
                clues: &t.clues,
            });
        }
    }
    out
}

/// What [`check`] examined.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatedBinding {
    /// Objectives and triggers read.
    pub acts: usize,
    /// Of those, acts that state a clue.
    pub stated: usize,
    /// Clues stated.
    pub clues: usize,
    /// Clues refused.
    pub refused: usize,
}

impl StatedBinding {
    /// The one line a validation prints about the stated clues.
    pub fn line(&self) -> String {
        format!(
            "clues stated (DW1136): {} of {} act(s) state {} clue(s); {} name nothing that \
             presents them",
            self.stated, self.acts, self.clues, self.refused
        )
    }
}

/// `DW1136` over every stated clue: each names a text that exists. A `sight`
/// clue's anchor is resolved at the build, where anchors have cells.
pub fn check(c: &Campaign) -> (Vec<Diagnostic>, StatedBinding) {
    let mut d = Vec::new();
    let mut b = StatedBinding {
        acts: c
            .quests
            .content
            .quests
            .iter()
            .map(|q| q.objectives.len())
            .sum::<usize>()
            + c.quests.content.triggers.len(),
        ..StatedBinding::default()
    };
    let objectives: BTreeMap<&str, &Objective> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| q.objectives.iter())
        .map(|o| (o.id().as_str(), o))
        .collect();
    let written = |s: Option<&str>| s.map(str::trim).is_some_and(|s| !s.is_empty());
    for s in stated(c) {
        b.stated += 1;
        for (i, clue) in s.clues.iter().enumerate() {
            b.clues += 1;
            let why = match clue {
                Clue::Hint { objective } => match objectives.get(objective.as_str()) {
                    None => Some(format!("no objective `{}` exists", objective.as_str())),
                    Some(o) if !written(o.hint()) || !written(o.title()) => Some(format!(
                        "objective `{}` has no `hint` on a journal page (a hint is shown under \
                         its `title`). Write the clue as that objective's `hint`, or name the \
                         text that does hold it",
                        objective.as_str()
                    )),
                    Some(_) => None,
                },
                Clue::Record { objective } => match objectives.get(objective.as_str()) {
                    None => Some(format!("no objective `{}` exists", objective.as_str())),
                    Some(o) if !written(o.record()) => Some(format!(
                        "objective `{}` has no `record`, so its finished page holds no line of \
                         its own. Write what the party learned there as that objective's \
                         `record`, or name the text that does hold it",
                        objective.as_str()
                    )),
                    Some(_) => None,
                },
                Clue::Sight { .. } => None,
            };
            if let Some(why) = why {
                b.refused += 1;
                d.push(Diagnostic::error(
                    DW_CLUE_NAMES_NOTHING,
                    "quests",
                    format!("{}/{i}", s.path),
                    format!(
                        "{} `{}` states the clue `{}` ({}), and {why}.",
                        s.what,
                        s.id,
                        clue.by(),
                        clue.named()
                    ),
                ));
            }
        }
    }
    (d, b)
}

// ---------------------------------------------------------------------------
// What is presented — the build's half (`DW0863`)
// ---------------------------------------------------------------------------

/// How one clue fared on one walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// The walk's label.
    pub walk: String,
    /// Whether the clue is presented before the act on it.
    pub presented: bool,
    /// How it is presented, or why it is not.
    pub how: String,
}

/// One stated clue, with its verdict on every walk whose main line performs
/// its act.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClueRow {
    /// The clue as a message names it.
    pub clue: String,
    /// Its `by` word.
    pub by: &'static str,
    /// Its verdicts, one per judging walk; empty for an optional act.
    pub verdicts: Vec<Verdict>,
}

/// One act that states a clue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActRow {
    /// The objective's or trigger's id.
    pub id: String,
    /// The walks whose main line performs it; empty for an optional act.
    pub main_on: Vec<String>,
    /// Its clues.
    pub clues: Vec<ClueRow>,
}

/// What [`check_presented`] examined, stated whichever way it went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClueBinding {
    /// Walks taken: the critical path and every realised branch.
    pub walks: usize,
    /// Objectives and triggers read.
    pub acts: usize,
    /// Of those, acts that state a clue.
    pub stated: usize,
    /// Clues stated.
    pub clues: usize,
    /// Of the stating acts, performed on the main line of some walk — judged.
    pub main: usize,
    /// Of the stating acts, on no main line — optional, owed nothing.
    pub optional: usize,
    /// Clue-on-walk judgements made.
    pub judged: usize,
    /// Of those, text clues (`hint`, `record`) presented.
    pub text_presented: usize,
    /// Of those, `sight` clues presented.
    pub sight_presented: usize,
    /// Of those, refused.
    pub refused: usize,
    /// Every stating act, in document order.
    pub rows: Vec<ActRow>,
}

impl ClueBinding {
    /// The one line a build prints about this rule.
    pub fn line(&self) -> String {
        format!(
            "DW0863 binding: {} of {} act(s) state {} clue(s) — {} on the main line of {} \
             walk(s), {} clue judgement(s): {} text clue(s) and {} sight clue(s) presented \
             before their act, {} refused; {} optional act(s) owed nothing; {} act(s) state no \
             clue and are owed nothing",
            self.stated,
            self.acts,
            self.clues,
            self.main,
            self.walks,
            self.judged,
            self.text_presented,
            self.sight_presented,
            self.refused,
            self.optional,
            self.acts - self.stated
        )
    }

    /// The ledger a build writes to `validation/clues.json`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": "DW0863",
            "walks": self.walks,
            "acts": self.acts,
            "stated": self.stated,
            "clues": self.clues,
            "main": self.main,
            "optional": self.optional,
            "examined": self.judged,
            "text_presented": self.text_presented,
            "sight_presented": self.sight_presented,
            "refused": self.refused,
            "rows": self.rows.iter().map(|r| serde_json::json!({
                "id": r.id,
                "line": if r.main_on.is_empty() { "optional" } else { "main" },
                "main_on": r.main_on,
                "clues": r.clues.iter().map(|c| serde_json::json!({
                    "by": c.by,
                    "clue": c.clue,
                    "verdicts": c.verdicts.iter().map(|v| serde_json::json!({
                        "walk": v.walk,
                        "presented": v.presented,
                        "how": v.how,
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }
}

/// **Why a target is not in sight from a cell**, or `None` when it is: past
/// [`SIGHT_BLOCKS`], or every one of the nine lines stopped, from the eye of a
/// body standing on `from`. `body` sights a body's two cells; otherwise the
/// target is its one cell.
pub fn unseen(world: &World, from: [i32; 3], target: [i32; 3], body: bool) -> Option<String> {
    use crate::compiler::framing::{CellBox, in_sight};
    let c = crate::compiler::nav::cell_center(from);
    let eye = [c[0], c[1] + EYE_HEIGHT, c[2]];
    let sight_box = if body {
        CellBox::spanning(target, [target[0], target[1] + 1, target[2]])
    } else {
        CellBox::cell(target)
    };
    let at = sight_box.centre();
    let d = ((eye[0] - at[0]).powi(2) + (eye[1] - at[1]).powi(2) + (eye[2] - at[2]).powi(2)).sqrt();
    if d > SIGHT_BLOCKS {
        return Some(format!(
            "{d:.1} blocks away, past the {SIGHT_BLOCKS} blocks sight reaches"
        ));
    }
    // The vantage is no occluder: the cell a step names and the eye's own
    // cell never stop a line; the target's cells are the box's.
    let eye_cell = [
        eye[0].floor() as i32,
        eye[1].floor() as i32,
        eye[2].floor() as i32,
    ];
    let lines = in_sight(world, eye, &sight_box, &[from, eye_cell]);
    (!lines.seen()).then(|| {
        let first = lines.centre_block.map_or_else(String::new, |cell| {
            format!(": the centre line is stopped first at {cell:?}")
        });
        format!("{d:.1} blocks away and hidden — all nine lines to it are stopped{first}")
    })
}

/// **Every stated clue is presented before its act** (`DW0863`), over the
/// plan's own walks. Every refusal is returned; the build reports the first as
/// its verdict and the rest as further errors.
pub fn check_presented(plan: &Plan, world: &World) -> (ClueBinding, Vec<Failure>) {
    judge(plan, world, crate::compiler::acts::walks(plan))
}

/// The objective whose completion alone sets each flag: `Some(id)` when every
/// `set-flag` of the flag hangs in one objective's completion bundle, `None`
/// when anything else can set it (another objective, a quest's completion, a
/// trigger, a dialogue option, any optional root).
fn sole_setters(c: &Campaign) -> BTreeMap<String, Option<String>> {
    let mut out: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut note = |flag: &str, by: Option<&str>| {
        out.entry(flag.to_string())
            .and_modify(|cur| {
                if cur.as_deref() != by {
                    *cur = None;
                }
            })
            .or_insert_with(|| by.map(str::to_string));
    };
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        let by = match &site.owner {
            delvewright_dsl::EffectRootOwner::ObjectiveComplete { objective, .. } => {
                Some(*objective)
            }
            _ => None,
        };
        for e in list {
            e.visit_deep(&mut |x| {
                if let Some(f) = x.set_flag() {
                    note(f.as_str(), by);
                }
            });
        }
    });
    for d in &c.dialogue.content.dialogues {
        for n in &d.nodes {
            for o in &n.options {
                for e in &o.effects {
                    if let Some(f) = e.set_flag() {
                        note(f.as_str(), None);
                    }
                }
            }
        }
    }
    out
}

/// One walk as the judgement reads it.
struct Walked<'w> {
    walk: &'w ActWalk,
    /// Objective id → its step.
    step_of: BTreeMap<&'w str, usize>,
    /// Step → the objective performed there.
    at_step: BTreeMap<usize, &'w str>,
    /// Step → the walk's place for it: where the party stands before it, and
    /// its own cell.
    place: BTreeMap<usize, (Option<[i32; 3]>, [i32; 3])>,
}

/// [`check_presented`] over the walks it is handed — the plan's own, or a
/// test's.
pub fn judge(plan: &Plan, world: &World, walks: &ActWalks) -> (ClueBinding, Vec<Failure>) {
    let c = plan.campaign;
    let stated = stated(c);
    let mut b = ClueBinding {
        walks: walks.walks.len(),
        acts: c
            .quests
            .content
            .quests
            .iter()
            .map(|q| q.objectives.len())
            .sum::<usize>()
            + c.quests.content.triggers.len(),
        stated: stated.len(),
        clues: stated.iter().map(|s| s.clues.len()).sum(),
        ..ClueBinding::default()
    };
    let mut refusals: Vec<Failure> = Vec::new();
    if stated.is_empty() {
        return (b, refusals);
    }
    let mut objectives: BTreeMap<&str, (&str, &Objective)> = BTreeMap::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            objectives.insert(o.id().as_str(), (q.id.as_str(), o));
        }
    }
    let triggers = &c.quests.content.triggers;
    let setters = sole_setters(c);
    let spine = c.quest_plan.content.spine();
    let walked: Vec<Walked<'_>> = walks
        .walks
        .iter()
        .map(|w| {
            let mut place = BTreeMap::new();
            for a in &w.acts {
                place.insert(a.step, (a.from, a.target));
            }
            for p in &w.passes {
                place.insert(p.step, (p.from, p.target));
            }
            Walked {
                walk: w,
                step_of: w.steps.iter().map(|(s, id)| (id.as_str(), *s)).collect(),
                at_step: w.steps.iter().map(|(s, id)| (*s, id.as_str())).collect(),
                place,
            }
        })
        .collect();

    for s in &stated {
        let mut row = ActRow {
            id: s.id.clone(),
            main_on: Vec::new(),
            clues: s
                .clues
                .iter()
                .map(|cl| ClueRow {
                    clue: cl.named(),
                    by: cl.by(),
                    verdicts: Vec::new(),
                })
                .collect(),
        };
        for wd in &walked {
            let w = wd.walk;
            // Where this walk performs the act: an objective at its own step,
            // a trigger where the path presses or passes it.
            let step = match s.what {
                "objective" => wd.step_of.get(s.id.as_str()).copied(),
                _ => w
                    .acts
                    .iter()
                    .filter(|a| {
                        matches!(&a.what, crate::compiler::acts::ActWhat::Trigger(t) if *t == s.id)
                    })
                    .map(|a| a.step)
                    .chain(
                        w.passes
                            .iter()
                            .filter(|p| p.trigger.as_deref() == Some(s.id.as_str()))
                            .map(|p| p.step),
                    )
                    .next(),
            };
            let Some(step) = step else { continue };
            // The objective a trigger is performed in front of carries it:
            // the trigger is on the main line when that objective is.
            let carrier: Option<&str> = match s.what {
                "objective" => Some(s.id.as_str()),
                _ => wd
                    .at_step
                    .range(step..)
                    .next()
                    .map(|(_, id)| *id)
                    .or_else(|| wd.at_step.values().next_back().copied()),
            };
            let on_main = |oid: &str| objectives.get(oid).is_some_and(|(q, _)| spine.contains(*q));
            if !carrier.is_some_and(on_main) {
                continue;
            }
            row.main_on.push(w.label.clone());

            let (events, ancestors) = match &w.gates {
                None => (&plan.region_events, &plan.strict_ancestor_steps),
                Some(g) => (&g.0, &g.1),
            };
            // Everything forced before the act: its strict ancestors, and the
            // sole setter of every flag it or they require, to a fixed point.
            let mut forced: BTreeSet<&str> = BTreeSet::new();
            let mut forced_steps: BTreeSet<usize> =
                ancestors.get(&step).cloned().unwrap_or_default();
            let mut frontier: Vec<Vec<&str>> = Vec::new();
            if let Some(t) = triggers.iter().find(|t| t.id.as_str() == s.id)
                && s.what == "trigger"
            {
                frontier.push(t.requires_flags.iter().map(|f| f.as_str()).collect());
                // A trigger takes what is forced before the objective it is
                // performed in front of: that objective's own flags too.
                if let Some((_, o)) = carrier.and_then(|x| objectives.get(x)) {
                    frontier.push(o.requires_flags().iter().map(|f| f.as_str()).collect());
                }
            } else if let Some((_, o)) = objectives.get(s.id.as_str()) {
                frontier.push(o.requires_flags().iter().map(|f| f.as_str()).collect());
            }
            let mut pending: Vec<&str> = forced_steps
                .iter()
                .filter_map(|st| wd.at_step.get(st).copied())
                .collect();
            loop {
                while let Some(oid) = pending.pop() {
                    if !forced.insert(oid) {
                        continue;
                    }
                    if let Some((_, o)) = objectives.get(oid) {
                        frontier.push(o.requires_flags().iter().map(|f| f.as_str()).collect());
                    }
                    if let Some(st) = wd.step_of.get(oid) {
                        forced_steps.insert(*st);
                        for a in ancestors.get(st).into_iter().flatten() {
                            forced_steps.insert(*a);
                            if let Some(x) = wd.at_step.get(a) {
                                pending.push(x);
                            }
                        }
                    }
                }
                let Some(flags) = frontier.pop() else { break };
                for f in flags {
                    if let Some(Some(x)) = setters.get(f)
                        && x != &s.id
                        && let Some((k, _)) = objectives.get_key_value(x.as_str())
                    {
                        pending.push(k);
                    }
                }
            }

            let armed_by = |oid: &str| {
                events.armed.is_empty()
                    || events
                        .armed
                        .iter()
                        .take(step + 1)
                        .any(|set| set.contains(oid))
            };
            for (ci, clue) in s.clues.iter().enumerate() {
                b.judged += 1;
                let verdict: Result<String, String> = match clue {
                    Clue::Record { objective } => {
                        let x = objective.as_str();
                        if forced.contains(x) {
                            Ok(format!("`{x}` is over before `{}` in every play order", s.id))
                        } else if wd.step_of.get(x).is_some_and(|xs| *xs < step) {
                            Err(format!(
                                "`{x}` comes before `{}` on this walk, but nothing forces that \
                                 order: a run may reach `{}` with `{x}` not done and its record \
                                 unread. Make `{x}` something the story cannot go on without — \
                                 name it in the `after` of `{}` (or of the objective it serves), \
                                 or have its completion alone set a flag the step requires",
                                s.id, s.id, s.id
                            ))
                        } else if wd.step_of.contains_key(x) {
                            Err(format!(
                                "`{x}` is performed after `{}` on this walk, so its record is \
                                 not yet written when the party reaches the step",
                                s.id
                            ))
                        } else {
                            Err(format!(
                                "`{x}` is not on the main line: no run is made to do it, so a \
                                 run may reach `{}` with its record unread. A clue an optional \
                                 detour shows does not count for a main-line step: make `{x}` \
                                 something the story cannot go on without, or move the clue to \
                                 a text the main line reads",
                                s.id
                            ))
                        }
                    }
                    Clue::Hint { objective } => {
                        let x = objective.as_str();
                        if x == s.id {
                            Ok("it is the step's own hint, on its open page".to_string())
                        } else if forced.contains(x) {
                            Ok(format!("`{x}` is over before `{}` in every play order", s.id))
                        } else if on_main(x) && wd.step_of.contains_key(x) && armed_by(x) {
                            Ok(format!(
                                "`{x}` is a main-line objective already started when the walk \
                                 reaches `{}`",
                                s.id
                            ))
                        } else if !on_main(x) || !wd.step_of.contains_key(x) {
                            Err(format!(
                                "`{x}` is not on the main line: no run is made to start it, so \
                                 a run may reach `{}` with its page never opened. A clue an \
                                 optional detour shows does not count for a main-line step",
                                s.id
                            ))
                        } else {
                            Err(format!(
                                "`{x}` has not started when the walk reaches `{}`: its page \
                                 opens only once its own gate holds. Let `{x}` start before the \
                                 step, or name a text the party has by then",
                                s.id
                            ))
                        }
                    }
                    Clue::Sight { of } => match plan.point_any(of.as_str()) {
                        None => {
                            refusals.push(Failure {
                                code: DW_CLUE_NAMES_NOTHING,
                                message: format!(
                                    "{} `{}` states the clue `sight` ({}) at {}/{ci}, and \
                                     anchor `{}` resolves to no cell in the built world. Name \
                                     an anchor a placed piece provides.",
                                    s.what,
                                    s.id,
                                    clue.named(),
                                    s.path,
                                    of.as_str()
                                ),
                            });
                            Err("its anchor resolves to no cell".to_string())
                        }
                        Some(target) => {
                            seen_before(world, events, wd, step, &forced_steps, target)
                        }
                    },
                };
                match &verdict {
                    Ok(_) if matches!(clue, Clue::Sight { .. }) => b.sight_presented += 1,
                    Ok(_) => b.text_presented += 1,
                    Err(why) => {
                        b.refused += 1;
                        refusals.push(Failure {
                            code: DW_CLUE_UNPRESENTED,
                            message: format!(
                                "on {}, the main line reaches {} `{}` before its clue is \
                                 presented: it states the clue `{}` ({}) at {}/{ci}, and {why}. \
                                 The engine confirms only that a stated clue is in the journal \
                                 or was in sight before a main-line step; an optional step is \
                                 owed nothing, and a step that states no clue is owed nothing.",
                                w.label,
                                s.what,
                                s.id,
                                clue.by(),
                                clue.named(),
                                s.path,
                            ),
                        });
                    }
                }
                row.clues[ci].verdicts.push(Verdict {
                    walk: w.label.clone(),
                    presented: verdict.is_ok(),
                    how: verdict.unwrap_or_else(|e| e),
                });
            }
        }
        if row.main_on.is_empty() {
            b.optional += 1;
        } else {
            b.main += 1;
        }
        b.rows.push(row);
    }
    (b, refusals)
}

/// Whether `target` is in sight from a cell the party must stand on before
/// the act at `step`: the walk's entry cell, the place of every step forced
/// before it, its own place, and the route into each — every cell judged in
/// the world the walk holds when it arrives there.
fn seen_before(
    world: &World,
    events: &crate::compiler::plan::RegionEvents,
    wd: &Walked<'_>,
    step: usize,
    forced_steps: &BTreeSet<usize>,
    target: [i32; 3],
) -> Result<String, String> {
    let arrivals: Vec<usize> = forced_steps
        .iter()
        .copied()
        .chain(std::iter::once(step))
        .filter(|s| wd.place.contains_key(s))
        .collect();
    let walked = |g: usize, s: usize| g == 0 || g < s;
    let (worlds, at) = crate::compiler::nav::worlds_at(world, events, &walked, &arrivals);
    let mut nearest: Option<(String, [i32; 3])> = None;
    let mut cells = 0usize;
    // The cells in the order the walk stands on them: the entry cell (where
    // the first positioned step is walked from), then each arrival's route.
    let mut vantages: Vec<([i32; 3], &World, String)> = Vec::new();
    if let Some(c) = wd.place.values().next().and_then(|(from, _)| *from) {
        vantages.push((c, world, "the entry cell".to_string()));
    }
    for (i, s) in arrivals.iter().enumerate() {
        let w: &World = worlds[at[i]].as_ref().unwrap_or(world);
        let (from, to) = wd.place[s];
        let route = from.and_then(|f| w.route_between(f, to)).unwrap_or_default();
        for c in route.into_iter().chain(from).chain(std::iter::once(to)) {
            vantages.push((c, w, format!("on the way to step {s}")));
        }
    }
    for (c, w, what) in vantages {
        cells += 1;
        match unseen(w, c, target, false) {
            None => return Ok(format!("{target:?} is in sight from {c:?}, {what}")),
            Some(why) => {
                if nearest.is_none() {
                    nearest = Some((why, c));
                }
            }
        }
    }
    let (why, from) = nearest.unwrap_or_else(|| ("seen from no cell".to_string(), target));
    Err(format!(
        "its cell {target:?} is in sight from none of the {cells} cell(s) the main line stands \
         on before the step (the entry cell, the place of each step the story forces first, \
         the step's own place, and the routes into them) — from {from:?} it is {why}. Move the \
         thing where the main line passes it, or state the text that tells the answer"
    ))
}
