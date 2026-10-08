//! **The presses that drive a numeric gate** (`DW0985`).
//!
//! An objective may wait on a datum that only the party's own presses move: a
//! valve puzzle whose `requires_state` reads a counter three `use` triggers
//! write, one of which resets it. Nothing on the quest DAG makes anybody press
//! a valve, and the forced walk ([`super::path_triggers`]) performs a trigger
//! only for the way it opens or the flag it pays — so a path built from those
//! two reasons alone walks up to the gated objective and waits forever.
//!
//! This module finds the presses. It replays, in path order, every press the
//! path performs and asks at each objective whether its numeric gate holds; if
//! not, it searches for the shortest sequence of presses that makes it hold and
//! hands the sequence to the path as `trigger` steps the bot performs. The bot
//! decides nothing: what to press, and in what order, is the plan's.
//!
//! ## What is modelled
//!
//! A datum is **driven** when the plan can name its value at every step of the
//! path: it is declared, no stake forfeits it, no loop counts it, it has one
//! holder on this walk (`party`, or `player` in a one-player delve), and every
//! write to it anywhere in the campaign is a top-level effect of a **pressable
//! trigger** — a `use` or `strike` on an anchor, which the harness performs by
//! hand and whose bundle prints a fired marker ([`super::trigger_may_be_performed`]).
//! Such a datum moves only when the path presses something, so its value is a
//! function of the presses alone.
//!
//! A press is replayed the way the datapack runs it: the trigger's own gate
//! first (`once`, flags, numeric terms), then its effects **in order**, each
//! behind its own `when` evaluated against the value the earlier effects of the
//! same bundle produced — the post-write reading `DW0527` warns about. Every
//! comparison is [`delvewright_dsl::StateCompare::holds`], the engine's one
//! statement of what a comparison means.
//!
//! ## The search, and its bound
//!
//! Breadth-first over the replay state (each driven datum's value, the flags the
//! presses set, the `once` triggers already spent), pressing candidates in
//! declaration order, so the sequence found is the shortest and the same on
//! every build (ADR-0006). The bound is [`MAX_PRESSES`] presses and
//! [`MAX_STATES`] distinct states. No sequence within it is `DW0985`, naming
//! the gate, the presses tried, and every value they can reach.
//!
//! ## What is withheld
//!
//! A gate term on a datum that is not driven is not this module's to judge. A
//! press whose effect reads a datum the plan cannot name at that step is not
//! taken; if a search fails having skipped one, it refuses nothing — the answer
//! depended on a value the plan does not know.

use super::*;

/// The most presses one search may schedule in front of one objective.
pub(crate) const MAX_PRESSES: usize = 64;

/// The most distinct replay states one search may visit.
pub(crate) const MAX_STATES: usize = 16_384;

/// Whether `t` is a trigger the harness presses by hand at a place: a `use` or
/// a `strike` on an anchor.
pub(crate) fn pressable(t: &EnvTrigger) -> bool {
    matches!(
        t.on,
        delvewright_dsl::TriggerOn::Use | delvewright_dsl::TriggerOn::Strike
    ) && t.at.is_some()
}

/// Whether `t` writes a datum in its own top-level effects.
pub(crate) fn writes_state_at_top(t: &EnvTrigger) -> bool {
    t.effects.iter().any(|e| e.writes_state().is_some())
}

/// The data every value of which the presses alone decide — see the module
/// doc for the rule.
fn driven_data(campaign: &Campaign) -> BTreeSet<String> {
    let content = &campaign.quests.content;
    let mut out: BTreeSet<String> = BTreeSet::new();
    for s in &content.state {
        let one_holder = s.scope == delvewright_dsl::StateScope::Party || min_players(campaign) < 2;
        if one_holder {
            out.insert(s.id.as_str().to_string());
        }
    }
    for s in &content.stakes {
        out.remove(s.state.as_str());
    }
    for l in &content.loops {
        if let Some(counts) = &l.counts {
            out.remove(counts.as_str());
        }
    }
    // Every write anywhere: a top-level write of a pressable trigger keeps the
    // datum, any other write (another root, a nested list, a trigger nobody
    // presses by hand) disqualifies it.
    let mut written_by_press: BTreeSet<String> = BTreeSet::new();
    for_each_effect_root(campaign, &mut |site, list| {
        let top_of_press = matches!(site.root, EffectRoot::Trigger(t) if pressable(t));
        for e in list {
            if let Some((id, _)) = e.writes_state() {
                if top_of_press {
                    written_by_press.insert(id.as_str().to_string());
                } else {
                    out.remove(id.as_str());
                }
            }
            for nested in e.nested_effect_lists() {
                strike_nested_writes(nested, &mut out);
            }
        }
    });
    out.retain(|id| written_by_press.contains(id));
    out
}

/// Remove every datum a nested list (at any depth) writes.
fn strike_nested_writes(list: &[QuestEffect], out: &mut BTreeSet<String>) {
    for e in list {
        if let Some((id, _)) = e.writes_state() {
            out.remove(id.as_str());
        }
        for nested in e.nested_effect_lists() {
            strike_nested_writes(nested, out);
        }
    }
}

/// One replay state of the search.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Held {
    /// Each driven datum's value.
    data: BTreeMap<String, i64>,
    /// Flags the presses set since the path began.
    flags: BTreeSet<String>,
    /// `once` triggers already performed.
    spent: BTreeSet<String>,
}

/// What a press did.
enum Pressed {
    /// The trigger's gate held and its bundle ran; the state after it.
    Ran(Held),
    /// The trigger's gate did not hold: nothing ran, and no marker prints.
    Refused,
    /// A term read a datum the plan cannot name here.
    Undecidable,
}

/// The context one press is replayed in.
struct Scene<'s> {
    initial: &'s BTreeMap<String, i64>,
    /// The flags the path holds walking up to this step.
    flags: &'s BTreeSet<String>,
    /// Data the plan does not drive, at the value the guaranteed replay names.
    known: &'s BTreeMap<String, Option<i64>>,
}

impl Scene<'_> {
    fn value(&self, h: &Held, id: &str) -> Option<i64> {
        match h.data.get(id) {
            Some(v) => Some(*v),
            None => self.known.get(id).copied().flatten(),
        }
    }

    fn flag(&self, h: &Held, f: &str) -> bool {
        self.flags.contains(f) || h.flags.contains(f)
    }

    /// Does `gate` hold at `h`? `None` when a term reads an unnamed datum.
    fn holds(
        &self,
        h: &Held,
        requires: &[delvewright_dsl::FlagId],
        forbids: &[delvewright_dsl::FlagId],
        cmps: &[delvewright_dsl::StateCompare],
    ) -> Option<bool> {
        if !requires.iter().all(|f| self.flag(h, f.as_str()))
            || forbids.iter().any(|f| self.flag(h, f.as_str()))
        {
            return Some(false);
        }
        let mut open = true;
        for c in cmps {
            let v = self.value(h, c.state.as_str())?;
            open &= i32::try_from(v).is_ok_and(|v| c.holds(v));
        }
        Some(open)
    }

    /// Press `t` at `h`, replaying its bundle line by line.
    fn press(&self, t: &EnvTrigger, h: &Held) -> Pressed {
        if t.once && h.spent.contains(t.id.as_str()) {
            return Pressed::Refused;
        }
        match self.holds(h, &t.requires_flags, &t.forbids_flags, &t.requires_state) {
            None => return Pressed::Undecidable,
            Some(false) => return Pressed::Refused,
            Some(true) => {}
        }
        let mut next = h.clone();
        if t.once {
            next.spent.insert(t.id.as_str().to_string());
        }
        for e in &t.effects {
            match self.holds(
                &next,
                e.requires_flags(),
                e.forbids_flags(),
                e.requires_state(),
            ) {
                None => return Pressed::Undecidable,
                Some(false) => continue,
                Some(true) => {}
            }
            if let delvewright_dsl::Verb::SetFlag { flag, .. } = &e.verb {
                next.flags.insert(flag.as_str().to_string());
            }
            if let Some((id, w)) = e.writes_state() {
                let id = id.as_str();
                // A write to a datum the plan does not drive cannot be
                // followed; the candidate filter keeps such a trigger out.
                let Some(v) = next.data.get(id).copied() else {
                    return Pressed::Undecidable;
                };
                let after = match w {
                    delvewright_dsl::StateWrite::Set(n) => i64::from(n),
                    delvewright_dsl::StateWrite::Add(n) => v + i64::from(n),
                    delvewright_dsl::StateWrite::Clear => {
                        self.initial.get(id).copied().unwrap_or(0)
                    }
                };
                next.data.insert(id.to_string(), after);
            }
        }
        Pressed::Ran(next)
    }
}

/// The shortest press sequence from `start` to a state where `goal` holds.
enum Search {
    Found(Vec<usize>, Held),
    /// No sequence within the bound; the values each goal datum reached, and
    /// whether a press was skipped as undecidable or the state bound was hit.
    Exhausted {
        reached: BTreeMap<String, BTreeSet<i64>>,
        undecidable: bool,
        capped: bool,
    },
}

fn search(
    scene: &Scene<'_>,
    candidates: &[&EnvTrigger],
    start: &Held,
    goal: &[&delvewright_dsl::StateCompare],
) -> Search {
    let done = |h: &Held| {
        goal.iter().all(|c| {
            h.data
                .get(c.state.as_str())
                .is_some_and(|v| i32::try_from(*v).is_ok_and(|v| c.holds(v)))
        })
    };
    let mut reached: BTreeMap<String, BTreeSet<i64>> = BTreeMap::new();
    let note = |h: &Held, reached: &mut BTreeMap<String, BTreeSet<i64>>| {
        for c in goal {
            if let Some(v) = h.data.get(c.state.as_str()) {
                reached
                    .entry(c.state.as_str().to_string())
                    .or_default()
                    .insert(*v);
            }
        }
    };
    note(start, &mut reached);
    let mut seen: BTreeSet<Held> = BTreeSet::new();
    seen.insert(start.clone());
    let mut queue: VecDeque<(Held, Vec<usize>)> = VecDeque::new();
    queue.push_back((start.clone(), Vec::new()));
    let mut undecidable = false;
    let mut capped = false;
    while let Some((h, seq)) = queue.pop_front() {
        if seq.len() >= MAX_PRESSES {
            continue;
        }
        for (k, t) in candidates.iter().enumerate() {
            let next = match scene.press(t, &h) {
                Pressed::Ran(n) => n,
                Pressed::Refused => continue,
                Pressed::Undecidable => {
                    undecidable = true;
                    continue;
                }
            };
            let mut s = seq.clone();
            s.push(k);
            if done(&next) {
                return Search::Found(s, next);
            }
            if seen.contains(&next) {
                continue;
            }
            if seen.len() >= MAX_STATES {
                capped = true;
                continue;
            }
            note(&next, &mut reached);
            seen.insert(next.clone());
            queue.push_back((next, s));
        }
    }
    Search::Exhausted {
        reached,
        undecidable,
        capped,
    }
}

/// The step a press of `t` is, standing at `pos`.
fn press_step(campaign: &Campaign, t: &EnvTrigger, pos: [i32; 3]) -> Step {
    Step::Trigger {
        trigger_id: t.id.as_str().to_string(),
        on: t.on.kind(),
        anchor_id: t.at_anchor().map(str::to_string),
        npc_id: None,
        assembly_id: None,
        pos,
        range: None,
        stand: None,
        block: pressed_block(campaign, t.id.as_str()),
    }
}

/// Where `t`'s anchor stands, looking in `area` first: a name another area
/// also declares is a candidate, never a match, so outside `area` only a
/// unique resolution is taken.
fn press_cell(anchors: &AnchorTable, t: &EnvTrigger, area: &str) -> Option<[i32; 3]> {
    let at = t.at_anchor()?;
    let cell = |r: &ResolvedAnchor| match r {
        ResolvedAnchor::Point { pos, .. } => *pos,
        ResolvedAnchor::Gate { from, .. } => *from,
    };
    if let Some(r) = anchors.get(&(area.to_string(), at.to_string())) {
        return Some(cell(r));
    }
    let mut hits = anchors.iter().filter(|((_, n), _)| n == at);
    let first = hits.next()?;
    if hits.next().is_some() {
        return None;
    }
    Some(cell(first.1))
}

/// **The presses each gated objective owes**, keyed like
/// [`super::path_triggers`]'s output by the path step they are performed in
/// front of, and performed after the triggers already due there.
///
/// `due` is what the path already performs; `data_before` is the guaranteed
/// replay's value of every datum walking up to each objective, read for the
/// data this module does not drive.
pub(super) fn drive_presses(
    campaign: &Campaign,
    anchors: &AnchorTable,
    path: &crate::compiler::flow::Playthrough,
    flags_at: &[BTreeSet<String>],
    due: &BTreeMap<usize, Vec<Step>>,
    data_before: &BTreeMap<String, BTreeMap<String, Option<i64>>>,
) -> Result<BTreeMap<usize, Vec<Step>>, PlanError> {
    let mut out: BTreeMap<usize, Vec<Step>> = BTreeMap::new();
    let driven = driven_data(campaign);
    if driven.is_empty() {
        return Ok(out);
    }
    let content = &campaign.quests.content;
    let initial: BTreeMap<String, i64> = content
        .state
        .iter()
        .filter(|s| driven.contains(s.id.as_str()))
        .map(|s| (s.id.as_str().to_string(), i64::from(s.initial)))
        .collect();
    // A candidate presses by hand and writes only data the plan follows.
    let candidates: Vec<&EnvTrigger> = content
        .triggers
        .iter()
        .filter(|t| pressable(t) && writes_state_at_top(t))
        .filter(|t| {
            t.effects.iter().all(|e| {
                e.writes_state()
                    .is_none_or(|(id, _)| driven.contains(id.as_str()))
            })
        })
        .collect();
    let by_id: BTreeMap<&str, &EnvTrigger> = content
        .triggers
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();
    let mut held = Held {
        data: initial.clone(),
        flags: BTreeSet::new(),
        spent: BTreeSet::new(),
    };
    let empty = BTreeMap::new();
    let no_flags = BTreeSet::new();
    for (si, st) in path.steps.iter().enumerate() {
        let known = data_before.get(&st.objective).unwrap_or(&empty);
        let scene = Scene {
            initial: &initial,
            flags: flags_at.get(si).unwrap_or(&no_flags),
            known,
        };
        // What the path already performs here moves the data first.
        for step in due.get(&si).into_iter().flatten() {
            let Some(t) = step.trigger().and_then(|id| by_id.get(id)) else {
                continue;
            };
            match scene.press(t, &held) {
                Pressed::Ran(n) => held = n,
                Pressed::Refused | Pressed::Undecidable => {
                    if t.once {
                        held.spent.insert(t.id.as_str().to_string());
                    }
                }
            }
        }
        let Some(obj) = objective_quest(campaign, &st.objective).map(|(_, o)| o) else {
            continue;
        };
        let goal: Vec<&delvewright_dsl::StateCompare> = obj
            .requires_state()
            .iter()
            .filter(|c| driven.contains(c.state.as_str()))
            .collect();
        let holds_now = goal.iter().all(|c| {
            held.data
                .get(c.state.as_str())
                .is_some_and(|v| i32::try_from(*v).is_ok_and(|v| c.holds(v)))
        });
        if goal.is_empty() || holds_now {
            continue;
        }
        let area = campaign
            .quest_plan
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == st.quest)
            .map(|q| q.area.as_str())
            .unwrap_or("");
        let placed: Vec<(&EnvTrigger, [i32; 3])> = candidates
            .iter()
            .filter_map(|t| press_cell(anchors, t, area).map(|p| (*t, p)))
            .collect();
        let pool: Vec<&EnvTrigger> = placed.iter().map(|(t, _)| *t).collect();
        match search(&scene, &pool, &held, &goal) {
            Search::Found(seq, end) => {
                let steps = out.entry(si).or_default();
                for k in seq {
                    let (t, pos) = placed[k];
                    steps.push(press_step(campaign, t, pos));
                }
                held = end;
            }
            Search::Exhausted {
                undecidable: true, ..
            } => {}
            Search::Exhausted {
                reached, capped, ..
            } => {
                let terms = goal
                    .iter()
                    .map(|c| format!("`{} {} {}`", c.state.as_str(), c.op.token(), c.value))
                    .collect::<Vec<_>>()
                    .join(", ");
                let now = goal
                    .iter()
                    .map(|c| c.state.as_str())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .map(|id| format!("`{id}` = {}", held.data.get(id).copied().unwrap_or(0)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let presses = if pool.is_empty() {
                    "no trigger the party presses by hand stands where the path can reach it"
                        .to_string()
                } else {
                    pool.iter()
                        .map(|t| format!("`{}`", t.id.as_str()))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let reach = reached
                    .iter()
                    .map(|(id, vs)| {
                        let vs: Vec<String> = vs.iter().take(16).map(i64::to_string).collect();
                        let more = if reached[id].len() > 16 { ", …" } else { "" };
                        format!("`{id}` ∈ {{{}{more}}}", vs.join(", "))
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                let bound = if capped {
                    format!("the search visited its bound of {MAX_STATES} distinct states")
                } else {
                    format!("every sequence of at most {MAX_PRESSES} presses was tried")
                };
                return Err(PlanError::new(
                    DW_GATE_UNDRIVABLE,
                    format!(
                        "objective `{}` waits on {terms}, and only presses write that datum. \
                         From where the path stands ({now}), pressing {presses} — each bundle \
                         replayed in order, every effect's `when` read against the value the \
                         lines before it produced — never satisfies the gate: {bound}, and the \
                         values reached were {reach}. The delve cannot be finished. Look for a \
                         bundle whose own write is followed by a line gated on the value it \
                         just produced (`DW0527` names each one): move every reading effect \
                         ahead of the write, or change the gate",
                        st.objective
                    ),
                ));
            }
        }
    }
    Ok(out)
}
