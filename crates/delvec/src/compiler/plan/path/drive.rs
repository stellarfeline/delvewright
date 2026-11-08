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
    /// Data the plan does not drive that a press has written: from then on the
    /// plan cannot name their value. Only a lenient scene records one.
    lost: BTreeSet<String>,
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
    /// Whether a write to a datum the plan does not drive is followed by
    /// forgetting that datum's value (`true`) rather than abandoning the press
    /// as undecidable (`false`). A flag-paying press (`DW1080`) is lenient: what
    /// it is replayed for is the flag, and a datum it writes on the side is
    /// only lost to the lines that read it later.
    lenient: bool,
}

impl Scene<'_> {
    fn value(&self, h: &Held, id: &str) -> Option<i64> {
        if h.lost.contains(id) {
            return None;
        }
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
            if self
                .nested_flags(&e.nested_effect_lists(), e, &mut next)
                .is_none()
            {
                return Pressed::Undecidable;
            }
            if let Some((id, w)) = e.writes_state() {
                let id = id.as_str();
                // A write to a datum the plan does not drive cannot be
                // followed; the candidate filter keeps such a trigger out,
                // and a lenient scene forgets the datum instead.
                let Some(v) = next.data.get(id).copied() else {
                    if self.lenient {
                        next.lost.insert(id.to_string());
                        continue;
                    }
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

    /// The flags the lists nested under `parent` set, each line behind its own
    /// gate read at the value the press holds now — the producer table's
    /// nesting rule: a reaction bundle (`on_respawn`, `on_rest`, `on_caught`)
    /// is not descended. A nested write is not applied; it already keeps its
    /// datum out of the driven set, and a lenient scene forgets the datum.
    /// `None` when a gate reads a datum the plan cannot name.
    fn nested_flags(
        &self,
        lists: &[&[QuestEffect]],
        parent: &QuestEffect,
        next: &mut Held,
    ) -> Option<()> {
        if matches!(
            parent.verb,
            delvewright_dsl::Verb::SetCheckpoint { .. }
                | delvewright_dsl::Verb::Bonfire { .. }
                | delvewright_dsl::Verb::BeginStealth { .. }
        ) {
            return Some(());
        }
        for list in lists {
            for e in *list {
                if !self.holds(
                    next,
                    e.requires_flags(),
                    e.forbids_flags(),
                    e.requires_state(),
                )? {
                    continue;
                }
                if let delvewright_dsl::Verb::SetFlag { flag, .. } = &e.verb {
                    next.flags.insert(flag.as_str().to_string());
                }
                if let Some((id, _)) = e.writes_state()
                    && self.lenient
                {
                    next.lost.insert(id.as_str().to_string());
                }
                self.nested_flags(&e.nested_effect_lists(), e, next)?;
            }
        }
        Some(())
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

/// Whether every term of `goal` holds at `h` — the objective goal of
/// [`search`].
fn terms_hold(h: &Held, goal: &[&delvewright_dsl::StateCompare]) -> bool {
    goal.iter().all(|c| {
        h.data
            .get(c.state.as_str())
            .is_some_and(|v| i32::try_from(*v).is_ok_and(|v| c.holds(v)))
    })
}

/// The shortest sequence of presses of `candidates` from `start` to a state
/// `done` accepts, noting every value each datum in `watch` reaches.
fn search(
    scene: &Scene<'_>,
    candidates: &[&EnvTrigger],
    start: &Held,
    done: &dyn Fn(&Held) -> bool,
    watch: &BTreeSet<&str>,
) -> Search {
    let mut reached: BTreeMap<String, BTreeSet<i64>> = BTreeMap::new();
    let note = |h: &Held, reached: &mut BTreeMap<String, BTreeSet<i64>>| {
        for id in watch {
            if let Some(v) = h.data.get(*id) {
                reached.entry((*id).to_string()).or_default().insert(*v);
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

/// The numeric terms that stand between a press of `t` and its `set-flag` of
/// `flag`: `t`'s own `requires_state`, and every `when` on the chain of
/// effects down to each line that sets it (a reaction bundle is not
/// descended, the producer table's rule). Empty when nothing numeric gates
/// the flag, or `t` does not set it.
pub(crate) fn flag_state_terms<'t>(
    t: &'t EnvTrigger,
    flag: &str,
) -> Vec<&'t delvewright_dsl::StateCompare> {
    fn walk<'t>(
        list: &'t [QuestEffect],
        flag: &str,
        above: &mut Vec<&'t delvewright_dsl::StateCompare>,
        out: &mut Vec<&'t delvewright_dsl::StateCompare>,
        found: &mut bool,
    ) {
        for e in list {
            let depth = above.len();
            above.extend(e.requires_state());
            if let delvewright_dsl::Verb::SetFlag { flag: f, .. } = &e.verb
                && f.as_str() == flag
            {
                *found = true;
                out.extend(above.iter().copied());
            }
            if !matches!(
                e.verb,
                delvewright_dsl::Verb::SetCheckpoint { .. }
                    | delvewright_dsl::Verb::Bonfire { .. }
                    | delvewright_dsl::Verb::BeginStealth { .. }
            ) {
                for nested in e.nested_effect_lists() {
                    walk(nested, flag, above, out, found);
                }
            }
            above.truncate(depth);
        }
    }
    let mut out: Vec<&delvewright_dsl::StateCompare> = Vec::new();
    let mut found = false;
    walk(&t.effects, flag, &mut Vec::new(), &mut out, &mut found);
    if !found || (out.is_empty() && t.requires_state.is_empty()) {
        return Vec::new();
    }
    let mut all: Vec<&delvewright_dsl::StateCompare> = t.requires_state.iter().collect();
    for c in out {
        if !all.contains(&c) {
            all.push(c);
        }
    }
    all
}

/// Whether a hit count alone opens every numeric gate on `t`'s `set-flag` of
/// each of `flags` (spec-0082 §3.3, [`super::triggers::count_needed`]): `t`
/// has no `requires_state` of its own, and every top-level line that sets the
/// flag behind a numeric `when` is opened by the count of `t`'s own presses
/// the path already performs. A flag set behind a numeric gate on a nested
/// line is not a count.
fn counted_opens(campaign: &Campaign, t: &EnvTrigger, flags: &[&str]) -> bool {
    if !t.requires_state.is_empty() {
        return false;
    }
    flags.iter().all(|flag| {
        let mut nested_gate = false;
        for e in &t.effects {
            for list in e.nested_effect_lists() {
                for x in list {
                    x.visit_deep(&mut |y| {
                        nested_gate |= matches!(&y.verb,
                            delvewright_dsl::Verb::SetFlag { flag: f, .. } if f.as_str() == *flag)
                            && !flag_state_terms(t, flag).is_empty();
                    });
                }
            }
        }
        !nested_gate
            && t.effects.iter().enumerate().all(|(i, e)| {
                !matches!(&e.verb, delvewright_dsl::Verb::SetFlag { flag: f, .. } if f.as_str() == *flag)
                    || e.requires_state().is_empty()
                    || super::triggers::count_needed(campaign, t, i).is_some()
            })
    })
}

/// A flag a path performs a trigger to pay (`Flow::trigger_debts`), and the
/// objective that reads it.
pub(super) struct Owed {
    /// The flag.
    pub flag: String,
    /// The first objective on the path that reads it.
    pub reader: String,
}

/// **The presses each gated objective and each state-gated flag owes**, and
/// the schedule they make with what the path already performs: `due` with
/// every press added in front of the step that owes it.
///
/// Two gates are driven, by one replay:
///
/// * an objective's `requires_state` on driven data (`DW0985`), by the presses
///   performed after the triggers already due at its step; and
/// * a flag the path performs a trigger to pay (`owed`, by trigger) whose
///   `set-flag` waits on a numeric term — the trigger's own `requires_state`
///   or a `when` on the line ([`flag_state_terms`]) — however many other
///   triggers write the datum it reads. The press the path performs for the
///   flag is replayed; if the flag is not set by it, the shortest sequence of
///   presses ending in the state where it is replaces it (the payer itself
///   among the candidates, at the place the path performs it). A gate a
///   count of the payer's own presses opens (spec-0082 §3.3) stands as the
///   path performs it. Otherwise the gate cannot be shown to hold, and the
///   flag is not credited: `DW1080`.
///
/// The same builder serves the exported path and every spec-0025 branch path,
/// so a branch run performs the presses its own flags owe.
///
/// `data_before` is the guaranteed replay's value of every datum walking up
/// to each objective, read for the data this module does not drive.
pub(super) fn drive_presses(
    campaign: &Campaign,
    anchors: &AnchorTable,
    path: &crate::compiler::flow::Playthrough,
    flags_at: &[BTreeSet<String>],
    due: &BTreeMap<usize, Vec<Step>>,
    owed: &BTreeMap<String, Vec<Owed>>,
    data_before: &BTreeMap<String, BTreeMap<String, Option<i64>>>,
) -> Result<BTreeMap<usize, Vec<Step>>, PlanError> {
    let mut out: BTreeMap<usize, Vec<Step>> = BTreeMap::new();
    let driven = driven_data(campaign);
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
        lost: BTreeSet::new(),
    };
    let empty = BTreeMap::new();
    let no_flags = BTreeSet::new();
    let no_steps: Vec<Step> = Vec::new();
    for (si, st) in path.steps.iter().enumerate() {
        let known = data_before.get(&st.objective).unwrap_or(&empty);
        let scene = Scene {
            initial: &initial,
            flags: flags_at.get(si).unwrap_or(&no_flags),
            known,
            lenient: false,
        };
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
        // What the path already performs here moves the data first.
        let here = due.get(&si).unwrap_or(&no_steps);
        let mut i = 0;
        while i < here.len() {
            let step = &here[i];
            let Some(t) = step.trigger().and_then(|id| by_id.get(id)).copied() else {
                out.entry(si).or_default().push(step.clone());
                i += 1;
                continue;
            };
            let run = here[i..]
                .iter()
                .take_while(|s| s.trigger() == Some(t.id.as_str()))
                .count();
            let gated: Vec<&Owed> = owed
                .get(t.id.as_str())
                .into_iter()
                .flatten()
                .filter(|o| !flag_state_terms(t, &o.flag).is_empty())
                .collect();
            if gated.is_empty() {
                for s in &here[i..i + run] {
                    match scene.press(t, &held) {
                        Pressed::Ran(n) => held = n,
                        Pressed::Refused | Pressed::Undecidable => {
                            if t.once {
                                held.spent.insert(t.id.as_str().to_string());
                            }
                        }
                    }
                    out.entry(si).or_default().push(s.clone());
                }
                i += run;
                continue;
            }
            let flags: Vec<&str> = gated.iter().map(|o| o.flag.as_str()).collect();
            // The flags this press pays are not yet held when the path
            // decides to perform it (the journal credits them from the step
            // that owes them, the replay's stance).
            let mut before: BTreeSet<String> = scene.flags.clone();
            for f in &flags {
                before.remove(*f);
            }
            let pay = Scene {
                initial: &initial,
                flags: &before,
                known,
                lenient: true,
            };
            let paid = |h: &Held| flags.iter().all(|f| h.flags.contains(*f));
            // The press as the path performs it.
            let mut h = held.clone();
            let mut undecidable = false;
            for _ in 0..run {
                match pay.press(t, &h) {
                    Pressed::Ran(n) => h = n,
                    Pressed::Refused => {}
                    Pressed::Undecidable => {
                        undecidable = true;
                        break;
                    }
                }
            }
            if !undecidable && paid(&h) {
                held = h;
                out.entry(si)
                    .or_default()
                    .extend(here[i..i + run].iter().cloned());
                i += run;
                continue;
            }
            // The presses that make the gate hold, ending in the state where
            // the flag is set. The payer stands where the path performs it.
            let mut pool: Vec<(&EnvTrigger, Step)> = placed
                .iter()
                .filter(|(c, _)| c.id != t.id)
                .map(|(c, p)| (*c, press_step(campaign, c, *p)))
                .collect();
            pool.push((t, step.clone()));
            let pool_t: Vec<&EnvTrigger> = pool.iter().map(|(c, _)| *c).collect();
            let terms: Vec<&delvewright_dsl::StateCompare> =
                flags.iter().flat_map(|f| flag_state_terms(t, f)).collect();
            let watch: BTreeSet<&str> = terms.iter().map(|c| c.state.as_str()).collect();
            match search(&pay, &pool_t, &held, &paid, &watch) {
                Search::Found(seq, end) => {
                    let steps = out.entry(si).or_default();
                    for k in seq {
                        steps.push(pool[k].1.clone());
                    }
                    held = end;
                }
                Search::Exhausted { undecidable: u, .. }
                    if (u || undecidable) && counted_opens(campaign, t, &flags) =>
                {
                    // A hit count (spec-0082 §3.3): the path performs the
                    // payer as often as its own count needs.
                    if t.once {
                        held.spent.insert(t.id.as_str().to_string());
                    }
                    out.entry(si)
                        .or_default()
                        .extend(here[i..i + run].iter().cloned());
                }
                Search::Exhausted {
                    reached,
                    undecidable: u,
                    capped,
                } => {
                    return Err(unpaid(
                        t,
                        &gated,
                        &terms,
                        &held,
                        &pool_t,
                        &reached,
                        capped,
                        u || undecidable,
                    ));
                }
            }
            i += run;
        }
        let Some(obj) = objective_quest(campaign, &st.objective).map(|(_, o)| o) else {
            continue;
        };
        let goal: Vec<&delvewright_dsl::StateCompare> = obj
            .requires_state()
            .iter()
            .filter(|c| driven.contains(c.state.as_str()))
            .collect();
        if goal.is_empty() || terms_hold(&held, &goal) {
            continue;
        }
        let pool: Vec<&EnvTrigger> = placed.iter().map(|(t, _)| *t).collect();
        let watch: BTreeSet<&str> = goal.iter().map(|c| c.state.as_str()).collect();
        match search(&scene, &pool, &held, &|h| terms_hold(h, &goal), &watch) {
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
                return Err(PlanError::new(
                    DW_GATE_UNDRIVABLE,
                    format!(
                        "objective `{}` waits on {terms}, and only presses write that datum. \
                         From where the path stands ({now}), pressing {presses} — each bundle \
                         replayed in order, every effect's `when` read against the value the \
                         lines before it produced — never satisfies the gate: {}, and the \
                         values reached were {}. The delve cannot be finished. Look for a \
                         bundle whose own write is followed by a line gated on the value it \
                         just produced (the shape `DW0527` describes): move every reading effect \
                         ahead of the write, or change the gate",
                        st.objective,
                        bound_phrase(capped),
                        reach_phrase(&reached),
                    ),
                ));
            }
        }
    }
    Ok(out)
}

/// How a search that found nothing ended.
fn bound_phrase(capped: bool) -> String {
    if capped {
        format!("the search visited its bound of {MAX_STATES} distinct states")
    } else {
        format!("every sequence of at most {MAX_PRESSES} presses was tried")
    }
}

/// Every value each watched datum reached, at most sixteen per datum.
fn reach_phrase(reached: &BTreeMap<String, BTreeSet<i64>>) -> String {
    if reached.is_empty() {
        return "none the plan can name".to_string();
    }
    reached
        .iter()
        .map(|(id, vs)| {
            let shown: Vec<String> = vs.iter().take(16).map(i64::to_string).collect();
            let more = if vs.len() > 16 { ", …" } else { "" };
            format!("`{id}` ∈ {{{}{more}}}", shown.join(", "))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// `DW1080`: the flag a path performs `t` to pay waits on a numeric gate no
/// press sequence the plan can follow makes hold.
#[allow(clippy::too_many_arguments)]
fn unpaid(
    t: &EnvTrigger,
    gated: &[&Owed],
    terms: &[&delvewright_dsl::StateCompare],
    held: &Held,
    pool: &[&EnvTrigger],
    reached: &BTreeMap<String, BTreeSet<i64>>,
    capped: bool,
    undecidable: bool,
) -> PlanError {
    let reads = gated
        .iter()
        .map(|o| format!("`{}` reads `{}`", o.reader, o.flag))
        .collect::<Vec<_>>()
        .join(", ");
    let gate = terms
        .iter()
        .map(|c| format!("`{} {} {}`", c.state.as_str(), c.op.token(), c.value))
        .collect::<Vec<_>>()
        .join(", ");
    let now = terms
        .iter()
        .map(|c| c.state.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|id| match held.data.get(id) {
            Some(v) if !held.lost.contains(id) => format!("`{id}` = {v}"),
            _ => format!("`{id}` unknown to the plan"),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let presses = pool
        .iter()
        .map(|t| format!("`{}`", t.id.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let why = if undecidable {
        "a line on the way reads a datum the plan cannot name at this step — something other \
         than a press the plan follows writes it, or a press writes one it does not follow — \
         so whether the gate holds is not a fact the proof has"
            .to_string()
    } else {
        format!(
            "no sequence makes it hold: {}, and the values reached were {}",
            bound_phrase(capped),
            reach_phrase(reached)
        )
    };
    PlanError::new(
        DW_FLAG_GATE_UNPAID,
        format!(
            "{reads}, and the path performs trigger `{}` to set it; that `set-flag` waits on \
             {gate} (the trigger's `requires_state` and every `when` on the line). From where \
             the path stands ({now}), pressing {presses} — each bundle replayed in order, every \
             `when` read against the value the lines before it produced — {why}. The proof \
             does not credit the flag, and the delve cannot be shown finishable. Make the \
             datum one only presses write (no quest bundle, nested line or unpressed trigger \
             writes it), or set the flag where its gate is known to hold",
            t.id.as_str()
        ),
    )
}
