//! **The journal keeps what was done** (spec-0137): which objectives are
//! recorded, the lines the story writes for them, the order a finished page
//! shows those lines in, what each story world's walk leaves written, and
//! `DW1104` — a line no story world can show.
//!
//! # The model
//!
//! Every `record-outcome` is one **site** ([`delvewright_dsl::record::outcome_sites`],
//! the one enumeration and the one numbering). A site is a party fact held
//! once it fires: one score per site, set when the story writes it and never
//! cleared. A finished page of an objective shows its title and its `record`,
//! then every line the party's play wrote, each its own appended page, in the
//! objective's **display order**: the order the story enforces between its
//! sites — site `a` before site `b` when some story world's walk fires `a`
//! before `b` and none fires `b` before `a` — with the document's order
//! between sites the story does not order.
//!
//! # What a world's walk writes
//!
//! A site fires in a story world's walk when its pointer is among the effects
//! the world's run fired (a quest bundle, an `on_expire`, an entry deed), or it
//! is an effect of the dialogue option a `talk-to` step takes. A site at an
//! ambient root the walk does not drive in order — a forced act's bundle, a
//! death, a loop's crossing, a strike's landing, a respawn — is
//! **undetermined** in every world: the ledger says `null` for its objective
//! there rather than guess. An option the walk does not take is written by no
//! walk; a player can still choose it, which `DW1104` counts.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::record::{OutcomeAt, OutcomeSite, outcome_sites};
use delvewright_dsl::{Campaign, Diagnostic, DwCode, EffectRootOwner, ExitTier, Objective};

use crate::compiler::flow::Flow;

delvewright_dsl::dw_code! {
    /// `DW1104`: a `record-outcome` whose line no story world can show — in
    /// every enumerated world either the site does not fire or the objective it
    /// names never settles (spec-0137 §8).
    pub const DW_WAY_UNREACHED: DwCode = DwCode::new("DW1104", ExitTier::Build);
}

/// One recorded objective (spec-0137 §3).
#[derive(Clone)]
pub struct Recorded<'a> {
    /// Quest index and objective index in stage 5.
    pub quest: usize,
    pub index: usize,
    /// The objective.
    pub objective: &'a Objective,
    /// Its sites, as indices into [`Records::sites`], in display order.
    pub order: Vec<usize>,
}

impl Recorded<'_> {
    /// The objective's id.
    pub fn id(&self) -> &str {
        self.objective.id().as_str()
    }
}

/// The campaign's records: every recorded objective in document order, and
/// every outcome site in walk order.
#[derive(Clone)]
pub struct Records<'a> {
    pub objectives: Vec<Recorded<'a>>,
    pub sites: Vec<OutcomeSite<'a>>,
}

/// How a site stands in one world's walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Firing {
    /// The walk fires it, at this position of the walk's own order.
    At(usize),
    /// The walk may fire it; when, or whether, is not modelled.
    Undetermined,
    /// The walk does not fire it.
    No,
}

/// How a recorded objective ends in one world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settles {
    Done,
    Expired,
    Open,
}

impl Settles {
    pub fn token(self) -> &'static str {
        match self {
            Settles::Done => "done",
            Settles::Expired => "expired",
            Settles::Open => "open",
        }
    }
}

/// One recorded objective in one world.
#[derive(Clone, Debug)]
pub struct Finished {
    /// Index into [`Records::objectives`].
    pub objective: usize,
    pub settles: Settles,
    /// The ordinals the walk leaves written, in display order; `None` when a
    /// site of the objective is undetermined in this walk.
    pub written: Option<Vec<usize>>,
}

/// One story world, as the records read it.
#[derive(Clone, Debug)]
pub struct WorldRecords {
    /// The world's index (`world/<k>`).
    pub world: usize,
    pub finished: Vec<Finished>,
    /// Per site (index into [`Records::sites`]): could a player's play of
    /// this world write it — the walk's firing, an undetermined one, or an
    /// option a player can choose.
    pub possible: Vec<bool>,
}

/// The flat 1-based index of option `option` of node `node` of `tree` — the
/// index a `talk-to` step's `talk_option` names.
fn flat_option(c: &Campaign, tree: usize, node: usize, option: usize) -> usize {
    let t = &c.dialogue.content.dialogues[tree];
    t.nodes[..node].iter().map(|n| n.options.len()).sum::<usize>() + option + 1
}

impl<'a> Records<'a> {
    /// The records of `c`, each objective's sites in walk order (no story
    /// read). [`Self::ordered`] puts them in display order.
    pub fn plain(c: &'a Campaign) -> Self {
        let sites = outcome_sites(c);
        let named: BTreeSet<&str> = sites.iter().map(|s| s.objective.as_str()).collect();
        let mut objectives = Vec::new();
        for (qi, q) in c.quests.content.quests.iter().enumerate() {
            for (oi, o) in q.objectives.iter().enumerate() {
                if o.record().is_none() && !named.contains(o.id().as_str()) {
                    continue;
                }
                let order = sites
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.objective == o.id())
                    .map(|(i, _)| i)
                    .collect();
                objectives.push(Recorded {
                    quest: qi,
                    index: oi,
                    objective: o,
                    order,
                });
            }
        }
        Records { objectives, sites }
    }

    /// The records of `c` with every objective's sites in display order,
    /// read off `flow`'s story worlds.
    pub fn ordered(c: &'a Campaign, flow: &Flow<'_>) -> Self {
        let mut r = Self::plain(c);
        if r.objectives.iter().all(|o| o.order.len() < 2) {
            return r;
        }
        // `before[(a, b)]`: some walk fires a before b.
        let mut before: BTreeSet<(usize, usize)> = BTreeSet::new();
        for k in 0..flow.story().worlds.len() {
            let fired = r.firings(c, flow, k);
            for (a, fa) in fired.iter().enumerate() {
                for (b, fb) in fired.iter().enumerate() {
                    if let (Firing::At(x), Firing::At(y)) = (fa, fb)
                        && x < y
                        && r.sites[a].objective == r.sites[b].objective
                    {
                        before.insert((a, b));
                    }
                }
            }
        }
        for o in &mut r.objectives {
            let mut left: Vec<usize> = o.order.clone();
            let mut out = Vec::new();
            while !left.is_empty() {
                // The first site, in document order, that nothing still left
                // is enforced before; the document's order when every one is.
                let pick = left
                    .iter()
                    .position(|&s| {
                        !left.iter().any(|&t| {
                            t != s && before.contains(&(t, s)) && !before.contains(&(s, t))
                        })
                    })
                    .unwrap_or(0);
                out.push(left.remove(pick));
            }
            o.order = out;
        }
        r
    }

    /// Whether any objective is recorded.
    pub fn any(&self) -> bool {
        !self.objectives.is_empty()
    }

    /// The site that is quest effect `e`, by identity.
    pub fn site_of_effect(&self, e: &delvewright_dsl::QuestEffect) -> Option<&OutcomeSite<'a>> {
        self.sites.iter().find(|s| s.is_effect(e))
    }

    /// The site that is dialogue effect `e`, by identity.
    pub fn site_of_option_effect(
        &self,
        e: &delvewright_dsl::DialogueEffect,
    ) -> Option<&OutcomeSite<'a>> {
        self.sites.iter().find(|s| s.is_option_effect(e))
    }

    /// The recorded objective with this id.
    pub fn objective(&self, id: &str) -> Option<&Recorded<'a>> {
        self.objectives.iter().find(|o| o.id() == id)
    }

    /// How every site stands in world `k`'s walk, aligned with
    /// [`Self::sites`].
    pub fn firings(&self, c: &Campaign, flow: &Flow<'_>, k: usize) -> Vec<Firing> {
        let story = flow.story();
        let run = &story.runs[k];
        let pt = &run.playthrough;
        // The walk's order: each step's deeds and bundle, an option's effects
        // first within its step.
        let mut pos: BTreeMap<String, usize> = BTreeMap::new();
        let mut options: BTreeMap<(String, usize), usize> = BTreeMap::new();
        let mut n = 0usize;
        let mut fired = run.fired_in_order.iter().peekable();
        for (i, step) in pt.steps.iter().enumerate() {
            if let Some(opt) = step.talk_option
                && !pt.expires(&step.objective)
                && let Some(Objective::TalkTo { npc, .. }) = c
                    .quests
                    .content
                    .quests
                    .iter()
                    .flat_map(|q| &q.objectives)
                    .find(|o| o.id().as_str() == step.objective)
            {
                n += 1;
                options.insert((npc.as_str().to_string(), opt), n);
            }
            while let Some((j, p)) = fired.peek() {
                if *j != i {
                    break;
                }
                n += 1;
                pos.entry(p.clone()).or_insert(n);
                fired.next();
            }
        }
        let forced = &story.forced;
        self.sites
            .iter()
            .map(|s| match &s.at {
                OutcomeAt::Option {
                    tree,
                    node,
                    option,
                    ..
                } => {
                    let npc = c.dialogue.content.dialogues[*tree].npc.as_str().to_string();
                    match options.get(&(npc, flat_option(c, *tree, *node, *option))) {
                        Some(&p) => Firing::At(p),
                        None => Firing::No,
                    }
                }
                OutcomeAt::Effect { owner, root, .. } => {
                    if let Some(&p) = pos.get(&s.path) {
                        return Firing::At(p);
                    }
                    match owner {
                        EffectRootOwner::ObjectiveComplete { .. }
                        | EffectRootOwner::QuestComplete { .. }
                        | EffectRootOwner::ObjectiveExpire { .. } => Firing::No,
                        other => match crate::compiler::ending::act_of_owner(other, root) {
                            // An act the walk performs in every world is
                            // driven where the path needs it, not in order.
                            Some(a) if forced.contains(&a) => Firing::Undetermined,
                            // An elective the world performs is an entry
                            // deed, already in the order; one it does not
                            // perform is not done.
                            Some(_) => Firing::No,
                            // A death, a crossing, a landing, a respawn.
                            None => Firing::Undetermined,
                        },
                    }
                }
            })
            .collect()
    }

    /// Every story world, as the records read it: how each recorded
    /// objective ends and what the walk leaves written.
    pub fn worlds(&self, c: &Campaign, flow: &Flow<'_>) -> Vec<WorldRecords> {
        let story = flow.story();
        let mut out = Vec::new();
        for k in 0..story.worlds.len() {
            let run = &story.runs[k];
            if run.incoherent.is_some() {
                continue;
            }
            let pt = &run.playthrough;
            let fired = self.firings(c, flow, k);
            let finished = self
                .objectives
                .iter()
                .enumerate()
                .map(|(oi, o)| {
                    let settles = if !pt.steps.iter().any(|s| s.objective == o.id()) {
                        Settles::Open
                    } else if pt.expires(o.id()) {
                        Settles::Expired
                    } else {
                        Settles::Done
                    };
                    let written = if o
                        .order
                        .iter()
                        .any(|&s| fired[s] == Firing::Undetermined)
                    {
                        None
                    } else {
                        Some(
                            o.order
                                .iter()
                                .filter(|&&s| matches!(fired[s], Firing::At(_)))
                                .map(|&s| self.sites[s].ordinal)
                                .collect(),
                        )
                    };
                    Finished {
                        objective: oi,
                        settles,
                        written,
                    }
                })
                .collect();
            let possible = self
                .sites
                .iter()
                .zip(&fired)
                .map(|(s, f)| match f {
                    Firing::At(_) | Firing::Undetermined => true,
                    Firing::No => match &s.at {
                        // A player may choose an answer the walk does not
                        // take, where its gate can hold in this world.
                        OutcomeAt::Option {
                            tree, node, option, ..
                        } => c.dialogue.content.dialogues[*tree].nodes[*node].options[*option]
                            .requires_flags
                            .iter()
                            .all(|f| run.flags.contains(f.as_str())),
                        // A bundle the world performs fires where its gate holds.
                        OutcomeAt::Effect { owner, root, effect } => match owner {
                            EffectRootOwner::ObjectiveComplete { .. }
                            | EffectRootOwner::QuestComplete { .. }
                            | EffectRootOwner::ObjectiveExpire { .. } => false,
                            other => {
                                crate::compiler::ending::act_of_owner(other, root)
                                    .is_some_and(|a| story.worlds[k].performs.contains(&a))
                                    && effect
                                        .requires_flags()
                                        .iter()
                                        .all(|f| run.flags.contains(f.as_str()))
                            }
                        },
                    },
                })
                .collect();
            out.push(WorldRecords {
                world: k,
                finished,
                possible,
            });
        }
        out
    }
}

/// **The binding of the records proof**: what it read, zeroes included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecordsBinding {
    /// Recorded objectives.
    pub objectives: usize,
    /// `record-outcome` sites.
    pub sites: usize,
    /// Coherent story worlds read.
    pub worlds: usize,
    /// Sites refused (`DW1104`).
    pub refused: usize,
}

impl RecordsBinding {
    /// The line printed on every run that reads a campaign with a recorded
    /// objective.
    pub fn line(&self) -> String {
        format!(
            "records binding: {} recorded objective(s), {} record-outcome site(s) judged over {} \
             story world(s), {} refused (DW1104)",
            self.objectives, self.sites, self.worlds, self.refused
        )
    }
}

/// **`DW1104`** (spec-0137 §8): every `record-outcome` is shown in some story
/// world — it can fire there and its objective settles there.
pub fn check(c: &Campaign) -> (Vec<Diagnostic>, RecordsBinding) {
    let mut d = Vec::new();
    let flow = Flow::new(c);
    let r = Records::plain(c);
    let mut b = RecordsBinding {
        objectives: r.objectives.len(),
        sites: r.sites.len(),
        ..RecordsBinding::default()
    };
    if r.sites.is_empty() {
        return (d, b);
    }
    let worlds = r.worlds(c, &flow);
    b.worlds = worlds.len();
    for (si, s) in r.sites.iter().enumerate() {
        let Some(oi) = r.objectives.iter().position(|o| o.id() == s.objective.as_str()) else {
            // An objective no quest declares is `DW0112`.
            continue;
        };
        let shown = worlds.iter().any(|w| {
            w.possible[si] && !matches!(w.finished[oi].settles, Settles::Open)
        });
        if !shown {
            b.refused += 1;
            let fires = worlds.iter().filter(|w| w.possible[si]).count();
            let settles = worlds
                .iter()
                .filter(|w| !matches!(w.finished[oi].settles, Settles::Open))
                .count();
            d.push(Diagnostic::error(
                DW_WAY_UNREACHED,
                s.stage,
                s.path.clone(),
                format!(
                    "this `record-outcome` writes a way `{}` ends, and no story world shows it: of \
                     {} world(s), it can fire in {fires} and `{}` settles in {settles}, and in none \
                     both. The line is a way the task ends that no play reaches. Put it on the \
                     event that decides that way, or make that event reachable.",
                    s.objective,
                    worlds.len(),
                    s.objective
                ),
            ));
        }
    }
    (d, b)
}
