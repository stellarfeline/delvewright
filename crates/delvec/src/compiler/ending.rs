//! **An ending reads the run** (spec-0108, `DW1029`, `DW1030`).
//!
//! An ending is a `campaign-complete`. What it reads is every gate in its
//! **final scene** — the whole bundle the act that fires it fires: the effect
//! root the `campaign-complete` stands in at any depth, with every list nested
//! under it — the ending's own gate and every other line's alike (§3.2 as
//! amended). A sibling gated on an input is the epilogue line
//! that states that thread's outcome, so counting siblings is what makes "the
//! epilogue states the outcome per thread" and "the ending reads the input" one
//! fact.
//!
//! This module holds the two halves of the reading:
//!
//! * the **static** half ([`Scenes::of`]): the endings, the scenes they stand
//!   in, each scene's inputs, every writer of every input with the kind of act
//!   that fires it (*said*, *did*, *finished*, §3.3) and the quest that dates
//!   it, and each scene's aftermath — the effects whose verb changes the world
//!   ([`delvewright_dsl::Verb::changes_world`], §3.7);
//! * the **verdicts** ([`analyse`]): which inputs are seeded (§3.4), each
//!   ending's count against the campaign's `endings_read` (`DW1029`, §7.1), and
//!   which story worlds reach each ending (`DW1030`, §7.2) — read off
//!   [`crate::compiler::flow::Flow::story`], the story-world enumeration of
//!   §4.
//!
//! Everything here is validation metadata: nothing this module computes reaches
//! the shipped datapack. The engine never reads an ending's prose (CLAUDE.md:
//! a check judges what the author declared, and never infers an undeclared
//! intent in order to refuse).

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{
    Campaign, Diagnostic, DialogueEffect, DwCode, EffectRootOwner, ExitTier, Objective,
    QuestEffect, StateCompare, Verb,
};

delvewright_dsl::dw_code! {
    /// An ending reads fewer seeded inputs (or fewer with a deed among their
    /// writers) than the campaign's `endings_read` declares every ending reads.
    pub const DW_ENDING_READS_LESS: DwCode = DwCode::new("DW1029", ExitTier::Analysis);
}

delvewright_dsl::dw_code! {
    /// A declared ending no story world reaches — or one whose reaching is
    /// unproven (the witness search's bound, or a gate read against a datum no
    /// world dates).
    pub const DW_ENDING_UNREACHED: DwCode = DwCode::new("DW1030", ExitTier::Analysis);
}

/// The id every artifact names an ending that states none by (§3.1).
pub const UNNAMED: &str = "ending/<unnamed>";

// ---------------------------------------------------------------------------
// the static half
// ---------------------------------------------------------------------------

/// One flag or datum a final scene's gates read.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputKey {
    /// A flag, read by `requires_flags` or `forbids_flags`.
    Flag(String),
    /// A datum, read by a `requires_state` term.
    Datum(String),
}

impl InputKey {
    /// The id as the document spells it (`flag/…`, `state/…`).
    pub fn id(&self) -> &str {
        match self {
            InputKey::Flag(f) | InputKey::Datum(f) => f,
        }
    }
}

/// The kind of act that fires a writer (§3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WriterKind {
    /// A dialogue option was chosen.
    Said,
    /// A thing was done in the world.
    Did,
    /// A quest was completed.
    Finished,
}

impl WriterKind {
    /// The token every artifact spells.
    pub fn token(self) -> &'static str {
        match self {
            WriterKind::Said => "said",
            WriterKind::Did => "did",
            WriterKind::Finished => "finished",
        }
    }

    /// Whether this act is a **deed** — done or finished, never said.
    pub fn is_deed(self) -> bool {
        !matches!(self, WriterKind::Said)
    }
}

/// One producer of an input (R4): a `set-flag` / datum write on any root, a
/// dialogue option's `set-flag`, a trap's or timed gate's `disarm.sets_flag`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Writer {
    /// The JSON pointer of the writing effect or field.
    pub path: String,
    /// The stage document it lives in.
    pub stage: &'static str,
    /// The kind of act that fires it.
    pub kind: WriterKind,
    /// The quest whose step dates it, when one does. `None` for an ambient
    /// root (a trigger, a trap, a shortcut, a shop offer, an `on_kill`, an
    /// assembly's `on_land`, `on_death`, a loop's `on_cross`) and for a dialogue
    /// option no `talk-to` objective reaches.
    pub quest: Option<String>,
    /// The act that fires it, by id, when it is one a story world may perform
    /// or skip: an optional quest (`quest/…`), a trigger, a trap's payload, a
    /// trap's or timed gate's disarm, a shortcut's unlock, a shop offer, an
    /// `on_kill`. Whether the act is elective or forced is the flow model's
    /// answer ([`crate::compiler::flow::StoryModel::is_elective`]).
    pub act: Option<String>,
}

/// One aftermath effect of a final scene (§3.7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aftermath {
    /// The effect's JSON pointer.
    pub path: String,
    /// Its verb's tag.
    pub verb: &'static str,
}

/// A final scene: one effect root whose bundle holds at least one
/// `campaign-complete`, at any depth — everything the act that fires it fires.
#[derive(Clone, Debug)]
pub struct Scene {
    /// The root list's JSON pointer (an element's is this plus `/<index>`).
    pub path: String,
    /// The stage document.
    pub stage: &'static str,
    /// The quest whose `on_complete` or `on_objective_complete` holds the list,
    /// at any depth; `None` for a scene on a non-quest root (§3.4).
    pub host_quest: Option<String>,
    /// Every flag and datum a gate in the scene reads, sorted.
    pub inputs: Vec<InputKey>,
    /// Every effect in the scene whose verb changes the world, in walk order.
    pub aftermath: Vec<Aftermath>,
    /// Every `requires_state` term in the scene, by datum — the cuts that
    /// divide each datum into classes (§4.2).
    pub terms: Vec<StateCompare>,
}

/// One ending: a `campaign-complete` and the scene it stands in.
#[derive(Clone, Debug)]
pub struct Ending {
    /// `ending/<kebab>`, or [`UNNAMED`].
    pub id: String,
    /// The `campaign-complete`'s JSON pointer.
    pub path: String,
    /// Index into [`Scenes::scenes`].
    pub scene: usize,
}

/// The static reading of a campaign's endings.
#[derive(Clone, Debug, Default)]
pub struct Scenes {
    /// Every `campaign-complete`, in effect-walk order.
    pub endings: Vec<Ending>,
    /// Every final scene, in the order its first ending was found.
    pub scenes: Vec<Scene>,
    /// Every writer of every input any scene reads, keyed by input, in walk
    /// order.
    pub writers: BTreeMap<InputKey, Vec<Writer>>,
}

impl Scenes {
    /// Read the campaign. Pure and deterministic: the effect roots come from
    /// the one enumeration ([`delvewright_dsl::for_each_effect_root`]) and
    /// nesting from [`QuestEffect::nested_effect_lists_labeled`].
    pub fn of(c: &Campaign) -> Scenes {
        let mut out = Scenes::default();
        let optional = c.quest_plan.content.optional();
        // Every effect root, with its path and the quest that dates it. A final
        // scene is the WHOLE bundle the act fires — the root list, with every list
        // nested under it — whenever a `campaign-complete` stands anywhere in it.
        let mut roots: Vec<(String, &'static str, Option<String>, &[QuestEffect])> = Vec::new();
        delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
            let quest = match site.owner {
                EffectRootOwner::ObjectiveComplete { quest, .. }
                | EffectRootOwner::QuestComplete { quest } => Some(quest.id.as_str().to_string()),
                _ => None,
            };
            roots.push((site.path.clone(), site.stage, quest, list));
        });
        for (path, stage, quest, list) in &roots {
            let mut ends: Vec<(String, &QuestEffect)> = Vec::new();
            for (i, e) in list.iter().enumerate() {
                completes_deep(e, &format!("{path}/{i}"), &mut ends);
            }
            if ends.is_empty() {
                continue;
            }
            let scene = out.scenes.len();
            let mut inputs: BTreeSet<InputKey> = BTreeSet::new();
            let mut aftermath = Vec::new();
            let mut terms = Vec::new();
            for (i, e) in list.iter().enumerate() {
                read_deep(
                    e,
                    &format!("{path}/{i}"),
                    &mut inputs,
                    &mut aftermath,
                    &mut terms,
                );
            }
            out.scenes.push(Scene {
                path: path.clone(),
                stage,
                host_quest: quest.clone(),
                inputs: inputs.into_iter().collect(),
                aftermath,
                terms,
            });
            for (at, e) in ends {
                let id = match &e.verb {
                    Verb::CampaignComplete {
                        ending: Some(ending),
                    } => ending.as_str().to_string(),
                    _ => UNNAMED.to_string(),
                };
                out.endings.push(Ending {
                    id,
                    path: at,
                    scene,
                });
            }
        }
        let read: BTreeSet<InputKey> = out
            .scenes
            .iter()
            .flat_map(|s| s.inputs.iter().cloned())
            .collect();
        for key in &read {
            out.writers.insert(key.clone(), Vec::new());
        }
        // Effect writers, every root, every depth — a reaction bundle is no
        // producer (R4).
        delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
            let (kind_of, quest, act) = root_reading(&site.owner, &site.path, &optional);
            let Some(kind_of) = kind_of else {
                return;
            };
            for (i, e) in list.iter().enumerate() {
                writers_deep(
                    e,
                    &format!("{}/{i}", site.path),
                    site.stage,
                    &kind_of,
                    quest.as_deref(),
                    act.as_deref(),
                    &read,
                    &mut out.writers,
                );
            }
        });
        // Dialogue options: said, dated by the quest of the objective the
        // option completes, else of the first `talk-to` that opens the tree.
        for (ti, tree) in c.dialogue.content.dialogues.iter().enumerate() {
            let first_talk = talk_quest(c, tree.npc.as_str(), None);
            for (ni, node) in tree.nodes.iter().enumerate() {
                for (oi, opt) in node.options.iter().enumerate() {
                    let completes = opt.effects.iter().find_map(|e| match e {
                        DialogueEffect::CompleteObjective { objective } => {
                            Some(objective.as_str().to_string())
                        }
                        _ => None,
                    });
                    let quest = match &completes {
                        Some(o) => talk_quest(c, tree.npc.as_str(), Some(o)),
                        None => first_talk.clone(),
                    };
                    let act = quest
                        .as_deref()
                        .filter(|q| optional.contains(q))
                        .map(str::to_string);
                    for (ei, e) in opt.effects.iter().enumerate() {
                        if let DialogueEffect::SetFlag { flag } = e {
                            let key = InputKey::Flag(flag.as_str().to_string());
                            if let Some(ws) = out.writers.get_mut(&key) {
                                ws.push(Writer {
                                    path: format!(
                                        "/content/dialogues/{ti}/nodes/{ni}/options/{oi}/effects/{ei}"
                                    ),
                                    stage: "dialogue",
                                    kind: WriterKind::Said,
                                    quest: quest.clone(),
                                    act: act.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
        // A disarm is a field, not an effect list: a thing done, ambient.
        for (i, t) in c.quests.content.traps.iter().enumerate() {
            if let Some(d) = &t.disarm {
                let key = InputKey::Flag(d.sets_flag.as_str().to_string());
                if let Some(ws) = out.writers.get_mut(&key) {
                    ws.push(Writer {
                        path: format!("/content/traps/{i}/disarm/sets_flag"),
                        stage: "quests",
                        kind: WriterKind::Did,
                        quest: None,
                        act: Some(disarm_act(t.id.as_str())),
                    });
                }
            }
        }
        for (i, g) in c.quests.content.timed_gates.iter().enumerate() {
            if let Some(d) = &g.disarm {
                let key = InputKey::Flag(d.sets_flag.as_str().to_string());
                if let Some(ws) = out.writers.get_mut(&key) {
                    ws.push(Writer {
                        path: format!("/content/timed_gates/{i}/disarm/sets_flag"),
                        stage: "quests",
                        kind: WriterKind::Did,
                        quest: None,
                        act: Some(disarm_act(g.id.as_str())),
                    });
                }
            }
        }
        out
    }

    /// Every scene's list pointer — what the flow replay snapshots the state
    /// at, the moment each list starts to fire.
    pub fn scene_paths(&self) -> BTreeSet<String> {
        self.scenes.iter().map(|s| s.path.clone()).collect()
    }

    /// The classes a datum's value divides into (§4.2): the intervals every
    /// `requires_state` term reading it in ANY final scene cuts the integers
    /// into, ascending.
    pub fn datum_classes(&self, id: &str) -> Vec<Interval> {
        let mut cuts: BTreeSet<i64> = BTreeSet::new();
        for s in &self.scenes {
            for t in &s.terms {
                if t.state.as_str() != id {
                    continue;
                }
                let v = i64::from(t.value);
                match t.op {
                    delvewright_dsl::CompareOp::AtLeast => {
                        cuts.insert(v);
                    }
                    delvewright_dsl::CompareOp::AtMost => {
                        cuts.insert(v + 1);
                    }
                    delvewright_dsl::CompareOp::Equals | delvewright_dsl::CompareOp::NotEquals => {
                        cuts.insert(v);
                        cuts.insert(v + 1);
                    }
                }
            }
        }
        let cuts: Vec<i64> = cuts.into_iter().collect();
        if cuts.is_empty() {
            return vec![Interval { lo: None, hi: None }];
        }
        let mut out = vec![Interval {
            lo: None,
            hi: Some(cuts[0] - 1),
        }];
        for w in cuts.windows(2) {
            out.push(Interval {
                lo: Some(w[0]),
                hi: Some(w[1] - 1),
            });
        }
        out.push(Interval {
            lo: cuts.last().copied(),
            hi: None,
        });
        out
    }

    /// The classes an input divides into: a flag's two values, a datum's
    /// intervals.
    pub fn classes(&self, key: &InputKey) -> Vec<Class> {
        match key {
            InputKey::Flag(_) => vec![Class::Set, Class::Unset],
            InputKey::Datum(id) => self
                .datum_classes(id)
                .into_iter()
                .map(Class::Within)
                .collect(),
        }
    }
}

/// An inclusive integer interval, open where a bound is `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Interval {
    /// The least value, or unbounded below.
    pub lo: Option<i64>,
    /// The greatest value, or unbounded above.
    pub hi: Option<i64>,
}

impl Interval {
    /// Whether `v` lies inside.
    pub fn contains(&self, v: i64) -> bool {
        self.lo.is_none_or(|l| v >= l) && self.hi.is_none_or(|h| v <= h)
    }

    /// The interval in words: `..=0`, `1`, `2..=4`, `3..`, `any`.
    pub fn words(&self) -> String {
        match (self.lo, self.hi) {
            (None, None) => "any".to_string(),
            (None, Some(h)) => format!("..={h}"),
            (Some(l), None) => format!("{l}.."),
            (Some(l), Some(h)) if l == h => format!("{l}"),
            (Some(l), Some(h)) => format!("{l}..={h}"),
        }
    }
}

/// One value class of one input (§4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// The flag is set.
    Set,
    /// The flag is not set.
    Unset,
    /// The datum lies inside the interval.
    Within(Interval),
}

impl Class {
    /// The class in words, as every artifact spells it.
    pub fn words(&self) -> String {
        match self {
            Class::Set => "set".to_string(),
            Class::Unset => "unset".to_string(),
            Class::Within(i) => i.words(),
        }
    }
}

/// Every `campaign-complete` at or under `e`, with its pointer, in walk order.
fn completes_deep<'a>(e: &'a QuestEffect, path: &str, out: &mut Vec<(String, &'a QuestEffect)>) {
    if matches!(e.verb, Verb::CampaignComplete { .. }) {
        out.push((path.to_string(), e));
    }
    for (pseg, _, inner) in e.nested_effect_lists_labeled() {
        for (j, x) in inner.iter().enumerate() {
            completes_deep(x, &format!("{path}/{pseg}/{j}"), out);
        }
    }
}

/// A scene's reading of one effect and everything nested under it.
fn read_deep(
    e: &QuestEffect,
    path: &str,
    inputs: &mut BTreeSet<InputKey>,
    aftermath: &mut Vec<Aftermath>,
    terms: &mut Vec<StateCompare>,
) {
    for f in e.requires_flags().iter().chain(e.forbids_flags()) {
        inputs.insert(InputKey::Flag(f.as_str().to_string()));
    }
    for t in e.requires_state() {
        inputs.insert(InputKey::Datum(t.state.as_str().to_string()));
        terms.push(t.clone());
    }
    if e.verb.changes_world() {
        aftermath.push(Aftermath {
            path: path.to_string(),
            verb: e.verb.tag(),
        });
    }
    for (pseg, _, inner) in e.nested_effect_lists_labeled() {
        for (j, x) in inner.iter().enumerate() {
            read_deep(x, &format!("{path}/{pseg}/{j}"), inputs, aftermath, terms);
        }
    }
}

/// How a writer on a root reads: the kind of its act (`None` for a root that
/// produces nothing — a dialogue `on_respawn`), the quest that dates it, and
/// the act a story world may perform or skip.
type KindOf = Box<dyn Fn(Option<&str>) -> WriterKind>;

fn root_reading(
    owner: &EffectRootOwner<'_>,
    path: &str,
    optional: &BTreeSet<&str>,
) -> (Option<KindOf>, Option<String>, Option<String>) {
    match owner {
        EffectRootOwner::ObjectiveComplete { quest, objective } => {
            let said = quest
                .objectives
                .iter()
                .find(|o| o.id().as_str() == *objective)
                .is_some_and(|o| matches!(o, Objective::TalkTo { .. }));
            let q = quest.id.as_str().to_string();
            let act = optional.contains(q.as_str()).then(|| q.clone());
            let kind = if said {
                WriterKind::Said
            } else {
                WriterKind::Did
            };
            (Some(Box::new(move |_| kind)), Some(q), act)
        }
        EffectRootOwner::QuestComplete { quest } => {
            let q = quest.id.as_str().to_string();
            let act = optional.contains(q.as_str()).then(|| q.clone());
            (Some(Box::new(|_| WriterKind::Finished)), Some(q), act)
        }
        EffectRootOwner::DialogueRespawn => (None, None, None),
        other => (
            Some(Box::new(|_| WriterKind::Did)),
            None,
            act_of_owner(other, path),
        ),
    }
}

/// The act an ambient root is, and the act a disarm is: the producer table's
/// one spelling (§4.3).
pub use delvewright_dsl::producers::{act_of_owner, disarm_act};

#[allow(clippy::too_many_arguments)]
fn writers_deep(
    e: &QuestEffect,
    path: &str,
    stage: &'static str,
    kind_of: &KindOf,
    quest: Option<&str>,
    act: Option<&str>,
    read: &BTreeSet<InputKey>,
    out: &mut BTreeMap<InputKey, Vec<Writer>>,
) {
    let key = if let Some(f) = e.set_flag() {
        Some(InputKey::Flag(f.as_str().to_string()))
    } else {
        e.writes_state()
            .map(|(id, _)| InputKey::Datum(id.as_str().to_string()))
    };
    if let Some(k) = key
        && read.contains(&k)
        && let Some(ws) = out.get_mut(&k)
    {
        ws.push(Writer {
            path: path.to_string(),
            stage,
            kind: kind_of(quest),
            quest: quest.map(str::to_string),
            act: act.map(str::to_string),
        });
    }
    // A reaction bundle fires when somebody dies, rests or is seen: no producer.
    if matches!(
        e.verb,
        Verb::SetCheckpoint { .. } | Verb::Bonfire { .. } | Verb::BeginStealth { .. }
    ) {
        return;
    }
    for (pseg, _, inner) in e.nested_effect_lists_labeled() {
        for (j, x) in inner.iter().enumerate() {
            writers_deep(
                x,
                &format!("{path}/{pseg}/{j}"),
                stage,
                kind_of,
                quest,
                act,
                read,
                out,
            );
        }
    }
}

/// The quest of the `talk-to` objective on `npc` — `objective` itself when
/// named, else the first one in declaration order.
fn talk_quest(c: &Campaign, npc: &str, objective: Option<&str>) -> Option<String> {
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            if let Objective::TalkTo { id, npc: n, .. } = o
                && n.as_str() == npc
                && objective.is_none_or(|x| x == id.as_str())
            {
                return Some(q.id.as_str().to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// the verdicts
// ---------------------------------------------------------------------------

/// One input of one ending, judged.
#[derive(Clone, Debug)]
pub struct InputVerdict {
    /// The flag or datum.
    pub key: InputKey,
    /// Its writers, with each one's kind and dated quest.
    pub writers: Vec<Writer>,
    /// Some writer is dated outside the host quest, or is ambient (§3.4).
    pub seeded: bool,
    /// Some writer is a deed (§3.3).
    pub deed: bool,
}

/// One ending, judged.
#[derive(Clone, Debug)]
pub struct EndingVerdict {
    /// The ending.
    pub ending: Ending,
    /// Its scene's host quest.
    pub host_quest: Option<String>,
    /// Every input, in key order.
    pub inputs: Vec<InputVerdict>,
    /// The story worlds whose run fires it, by canonical index.
    pub worlds: Vec<usize>,
    /// Worlds whose run fires it only on a gate term read against a datum no
    /// world dates — played, never proven.
    pub undecided: Vec<usize>,
}

impl EndingVerdict {
    /// Seeded inputs.
    pub fn seeded(&self) -> usize {
        self.inputs.iter().filter(|i| i.seeded).count()
    }

    /// Seeded inputs with a deed among their writers.
    pub fn seeded_deeds(&self) -> usize {
        self.inputs.iter().filter(|i| i.seeded && i.deed).count()
    }
}

/// The whole reading: the static scenes, each ending judged, and the story
/// worlds they were judged over.
pub struct Analysis {
    /// The static half.
    pub scenes: Scenes,
    /// Every ending, judged, in walk order.
    pub endings: Vec<EndingVerdict>,
}

/// Judge every ending of `c` over the story worlds `story` enumerates
/// ([`crate::compiler::flow::Flow::story`]).
pub fn analyse(c: &Campaign, story: &crate::compiler::flow::StoryModel) -> Analysis {
    let scenes = Scenes::of(c);
    let mut endings = Vec::new();
    for e in &scenes.endings {
        let scene = &scenes.scenes[e.scene];
        let host = scene.host_quest.clone();
        let inputs = scene
            .inputs
            .iter()
            .map(|k| {
                let writers = scenes.writers.get(k).cloned().unwrap_or_default();
                let seeded = writers.iter().any(|w| match (&w.quest, &host) {
                    (Some(q), Some(h)) => q != h,
                    // An ambient writer, or an ending with no host quest: no
                    // step of the host quest dates it.
                    _ => true,
                });
                let deed = writers.iter().any(|w| w.kind.is_deed());
                InputVerdict {
                    key: k.clone(),
                    writers,
                    seeded,
                    deed,
                }
            })
            .collect();
        let (worlds, undecided) = story.reaching(&e.path);
        endings.push(EndingVerdict {
            ending: e.clone(),
            host_quest: host,
            inputs,
            worlds,
            undecided,
        });
    }
    Analysis { scenes, endings }
}

/// `DW1029` and `DW1030` over an analysis.
pub fn check(
    c: &Campaign,
    story: &crate::compiler::flow::StoryModel,
    a: &Analysis,
) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    if let Some(decl) = c.quest_plan.content.endings_read {
        for v in &a.endings {
            for (term, declared, count) in [
                ("at_least", decl.at_least, v.seeded()),
                ("deeds_at_least", decl.deeds_at_least, v.seeded_deeds()),
            ] {
                let Some(n) = declared else { continue };
                if count >= n as usize {
                    continue;
                }
                d.push(Diagnostic::error(
                    DW_ENDING_READS_LESS,
                    "quests",
                    v.ending.path.clone(),
                    format!(
                        "`{id}` reads {count} {what}, and the campaign declares every ending \
                         reads at least {n} (`endings_read.{term}`). Its inputs: {inputs}. \
                         Remedy: gate a line of the final scene on a flag or datum a deed in \
                         another quest writes, or lower the declaration",
                        id = v.ending.id,
                        what = if term == "at_least" {
                            "seeded input(s)"
                        } else {
                            "seeded input(s) a deed writes"
                        },
                        inputs = inputs_words(v),
                    ),
                ));
            }
        }
    }
    // `DW0201` (no world completes the finale) and the critical path's `DW0204`
    // already name the cause of an ending nothing reaches; a second refusal
    // about the one break is withheld, as `DW0879` withholds itself.
    let flow = crate::compiler::flow::Flow::new(c);
    let mainline = flow.playthrough();
    if mainline.degenerate || flow.replay(&mainline).is_err() {
        return d;
    }
    for v in &a.endings {
        let reasons = story.reasons_for(&v.ending.path);
        let at_bound = story.at_bound;
        let fault = if !v.worlds.is_empty() {
            continue;
        } else if !v.undecided.is_empty() {
            format!(
                "it fires in {} only on a gate term read against a datum no story world dates \
                 ({}) — refused as unproven, never called clean",
                worlds_words(&v.undecided),
                story.undatable_words(&v.ending.path)
            )
        } else if at_bound > 0 {
            format!(
                "no enumerated world fires it, and the enumeration stopped at the bound of {} \
                 world(s) with {at_bound} target(s) unexamined — refused as unproven, never \
                 called clean",
                story.bound
            )
        } else {
            "no story world fires it".to_string()
        };
        d.push(Diagnostic::error(
            DW_ENDING_UNREACHED,
            "quests",
            v.ending.path.clone(),
            format!(
                "`{id}` is an ending no story world reaches: {fault}. Its gate: {gate}. \
                 Unrealizable target(s): {reasons}. Remedy: give the input an elective writer \
                 the proof can perform, loosen the interval, or delete the ending",
                id = v.ending.id,
                gate = gate_words(c, &v.ending.path),
                reasons = if reasons.is_empty() {
                    "none".to_string()
                } else {
                    reasons.join("; ")
                },
            ),
        ));
    }
    d
}

/// Each input in words: `<key>: <kinds>, <seeded|final-scene>, written at <…>`.
pub fn inputs_words(v: &EndingVerdict) -> String {
    if v.inputs.is_empty() {
        return "none".to_string();
    }
    v.inputs
        .iter()
        .map(|i| {
            let kinds: BTreeSet<&str> = i.writers.iter().map(|w| w.kind.token()).collect();
            let at: BTreeSet<String> = i
                .writers
                .iter()
                .map(|w| w.quest.clone().unwrap_or_else(|| "ambient".to_string()))
                .collect();
            format!(
                "{}: {}, {}, written at {}",
                i.key.id(),
                if kinds.is_empty() {
                    "no writer".to_string()
                } else {
                    kinds.into_iter().collect::<Vec<_>>().join("|")
                },
                if i.seeded { "seeded" } else { "final-scene" },
                if at.is_empty() {
                    "nothing".to_string()
                } else {
                    at.into_iter().collect::<Vec<_>>().join(", ")
                }
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn worlds_words(ws: &[usize]) -> String {
    ws.iter()
        .map(|k| format!("world/{k}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The ending's own gate, in words.
fn gate_words(c: &Campaign, path: &str) -> String {
    let mut out = "none (always open)".to_string();
    delvewright_dsl::for_each_campaign_effect(c, &mut |p, _site, e| {
        if p != path {
            return;
        }
        let mut parts: Vec<String> = Vec::new();
        for f in e.requires_flags() {
            parts.push(format!("requires {}", f.as_str()));
        }
        for f in e.forbids_flags() {
            parts.push(format!("forbids {}", f.as_str()));
        }
        for t in e.requires_state() {
            parts.push(format!("{} {} {}", t.state.as_str(), t.op.token(), t.value));
        }
        if !parts.is_empty() {
            out = parts.join(", ");
        }
    });
    out
}

// ---------------------------------------------------------------------------
// what an aftermath effect leaves to look at (§6.2)
// ---------------------------------------------------------------------------

/// What a proof can look at after the scene for an aftermath verb (§6.2):
/// `Some(kind)` names the observable, `None` is **emission-only** — a fake
/// player holds no inventory a template can read and is undamageable, a
/// volley's projectiles are gone, an atmosphere and a clip are client state.
pub fn observable(verb: &str) -> Option<&'static str> {
    match verb {
        "set-block" | "fill-region" | "clear-region" | "collapse" | "open-gate" | "close-gate"
        | "open-way" => Some("block"),
        "teleport" => Some("position"),
        "spawn-npc" | "spawn-actor" | "spawn-wave" | "spawn-assembly" | "lightning" => {
            Some("present")
        }
        "despawn-npc" | "despawn-actor" | "despawn-assembly" => Some("absent"),
        "move-npc" | "move-actor" | "unleash-actor" => Some("destination"),
        "set-time" => Some("time"),
        "set-weather" => Some("weather"),
        "set-flag" | "set-state" | "add-state" | "clear-state" | "set-checkpoint" | "bonfire"
        | "begin-stealth" | "end-stealth" | "arm-strikes" => Some("score"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// the binding line (§6.1)
// ---------------------------------------------------------------------------

/// What the ending reading examined, zeroes included (§6.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EndingBinding {
    /// Endings, `ending/<unnamed>` included.
    pub endings: usize,
    /// Inputs read, summed over endings.
    pub inputs: usize,
    /// Of those, seeded.
    pub seeded: usize,
    /// Of the seeded, with a deed among their writers.
    pub deeds: usize,
    /// Read only in the final scene.
    pub final_only: usize,
    /// Story worlds enumerated.
    pub worlds: usize,
    /// Targets no world realizes.
    pub unrealizable: usize,
    /// Targets or candidates the search left at the bound.
    pub at_bound: usize,
    /// Aftermath effects, summed over endings.
    pub aftermath: usize,
    /// Of those, observable after the scene.
    pub observable: usize,
    /// One line per ending.
    pub per_ending: Vec<String>,
}

impl EndingBinding {
    /// The campaign's binding line.
    pub fn line(&self) -> String {
        format!(
            "ending binding: {} ending(s); {} input(s) read, {} seeded ({} with a deed among \
             their writers), {} final-scene only; {} story world(s) enumerated ({} unrealizable \
             target(s), {} at the bound); {} aftermath effect(s), {} of them observable after \
             the scene.",
            self.endings,
            self.inputs,
            self.seeded,
            self.deeds,
            self.final_only,
            self.worlds,
            self.unrealizable,
            self.at_bound,
            self.aftermath,
            self.observable,
        )
    }

    /// The binding line, then one line per ending.
    pub fn lines(&self) -> Vec<String> {
        std::iter::once(self.line())
            .chain(self.per_ending.iter().cloned())
            .collect()
    }
}

/// The binding of an analysis over its story worlds.
pub fn binding(a: &Analysis, story: &crate::compiler::flow::StoryModel) -> EndingBinding {
    let mut b = EndingBinding {
        endings: a.endings.len(),
        worlds: story.worlds.len(),
        unrealizable: story.unrealizable(),
        at_bound: story.at_bound,
        ..EndingBinding::default()
    };
    for v in &a.endings {
        b.inputs += v.inputs.len();
        b.seeded += v.seeded();
        b.deeds += v.seeded_deeds();
        b.final_only += v.inputs.iter().filter(|i| !i.seeded).count();
        let scene = &a.scenes.scenes[v.ending.scene];
        b.aftermath += scene.aftermath.len();
        b.observable += scene
            .aftermath
            .iter()
            .filter(|x| observable(x.verb).is_some())
            .count();
        let verbs: Vec<&str> = scene.aftermath.iter().map(|x| x.verb).collect();
        let reached = if v.worlds.is_empty() {
            "no world".to_string()
        } else {
            worlds_words(&v.worlds)
        };
        b.per_ending.push(format!(
            "{}: reads {} input(s) — {}; aftermath: {} effect(s) [{}]; reached by {}",
            v.ending.id,
            v.inputs.len(),
            inputs_words(v),
            scene.aftermath.len(),
            verbs.join(" "),
            reached
        ));
    }
    b
}

/// `DW1029` and `DW1030` with what the reading examined — the one call the
/// validation funnel makes.
pub fn check_bound(c: &Campaign) -> (Vec<Diagnostic>, EndingBinding) {
    let flow = crate::compiler::flow::Flow::new(c);
    let story = flow.story();
    let a = analyse(c, story);
    let d = check(c, story, &a);
    let b = binding(&a, story);
    (d, b)
}
