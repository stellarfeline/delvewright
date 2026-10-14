//! **Whether one beat is certain to have run before another** (spec-0110 §5.6).
//!
//! A body's lifetime is judged by three rules — a `talk-to` on an NPC a
//! prerequisite removed (`DW0195`), a `talk-to` that activates before its
//! deferred NPC can exist (`DW0198`), and a shot of a body that is not there
//! to be seen (`DW1034`). All three ask one question of the campaign: *is beat
//! A certain to have run, on every playthrough, by the time beat B runs?*
//! [`Order::ordered_before`] is the one answer; [`Order::presence`] is the
//! body-lifetime question built on it.
//!
//! ## The model
//!
//! A beat is an effect (named by its address in the campaign — the same
//! effect value the caller holds), the moment an objective **activates**, or
//! world init. A is ordered before B when A runs unconditionally once its root
//! fires — no flag or state gate on A or on any effect it is nested in, and no
//! reaction bundle (`on_respawn`, `on_rest`, `on_caught`) on the way down,
//! because those are branches and count for nothing — and one of:
//!
//! - **the same effect list, an earlier index** — A is a sibling of B or of the
//!   effect B is nested in (a `move-*`'s `on_arrive` fires after its move
//!   starts; a `sequence` step at or after the step's start);
//! - **an earlier step of the same `sequence`** — by `at_ticks`, declaration
//!   order breaking a tie;
//! - **an earlier root on the quest graph** — A in a quest's `on_complete` or
//!   any of its `on_objective_complete` bundles, and B in a quest that depends
//!   on it, transitively; A in an objective's bundle and B in the same quest's
//!   `on_complete`, or in the bundle (or the activation) of an objective that
//!   lists it in `after`, transitively.
//!
//! An objective activates when its quest starts and its `after` prerequisites
//! complete, so its activation is preceded by those roots and precedes its own
//! bundle, its quest's `on_complete`, the bundles of objectives that wait on
//! it, and every quest that depends on its quest. Everything else — a trigger,
//! a trap, a shop, a dialogue bundle — fires when a player makes it fire and is
//! ordered only within its own list.

use std::collections::{BTreeMap, BTreeSet};

use crate::effects::{EffectRootOwner, for_each_effect_root};
use crate::envelope::Campaign;
use crate::{QuestEffect, Verb};

/// A moment the order is asked about.
#[derive(Clone, Copy, Debug)]
pub enum Beat<'a> {
    /// World init: before every effect.
    Setup,
    /// An effect, by address — the same value the campaign holds.
    Effect(&'a QuestEffect),
    /// The tick an objective activates.
    Activation {
        /// The owning quest.
        quest: &'a str,
        /// The objective.
        objective: &'a str,
    },
}

/// Where an effect's root list hangs.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Root {
    /// `on_objective_complete[objective]` of `quest`.
    Objective { quest: String, objective: String },
    /// `on_complete` of `quest`.
    QuestComplete { quest: String },
    /// Every other root: ordered only within itself.
    Ambient,
}

/// One step down from a list into an effect.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Link {
    /// Index within the list.
    index: usize,
    /// The list is a `sequence` step: `(at_ticks, step index)`.
    step: Option<(u32, usize)>,
}

/// An effect's address: its root and the links down to it.
#[derive(Clone, Debug)]
struct Loc {
    /// The root list's JSON pointer (a root's identity).
    root_path: String,
    root: Root,
    /// The links from the root list down to the effect.
    chain: Vec<Link>,
    /// JSON pointer to the effect.
    path: String,
    /// Runs on every playthrough that runs its root: no gate on it or above
    /// it, no reaction bundle on the way down.
    certain: bool,
}

/// What a body subject's lifetime resolves to at a beat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presence {
    /// Placed before the beat and not removed between: the placing beat's
    /// JSON pointer, or `"setup"`.
    Present(String),
    /// No placing is ordered before the beat. Carries every placing found
    /// (JSON pointers) — empty when nothing places the body at all.
    NeverPlaced(Vec<String>),
    /// A placing is ordered before the beat, and so is a removal after it:
    /// `(placing, removal)`.
    Removed(String, String),
}

/// The campaign's effects, indexed for ordering questions.
pub struct Order<'a> {
    campaign: &'a Campaign,
    locs: BTreeMap<usize, Loc>,
    ancestors: BTreeMap<String, BTreeSet<String>>,
    /// Per quest: objective → its transitive `after` prerequisites.
    prereqs: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
}

fn key(e: &QuestEffect) -> usize {
    e as *const QuestEffect as usize
}

fn gated(e: &QuestEffect) -> bool {
    !e.requires_flags().is_empty() || !e.forbids_flags().is_empty() || !e.requires_state().is_empty()
}

impl<'a> Order<'a> {
    /// Index every effect of every root of `c`.
    pub fn of(c: &'a Campaign) -> Order<'a> {
        let mut locs = BTreeMap::new();
        for_each_effect_root(c, &mut |site, list| {
            let root = match &site.owner {
                EffectRootOwner::ObjectiveComplete { quest, objective } => Root::Objective {
                    quest: quest.id.as_str().to_string(),
                    objective: objective.to_string(),
                },
                EffectRootOwner::QuestComplete { quest } => Root::QuestComplete {
                    quest: quest.id.as_str().to_string(),
                },
                _ => Root::Ambient,
            };
            index_list(
                list,
                &site.path,
                &site.path,
                &root,
                &[],
                None,
                true,
                &mut locs,
            );
        });
        let ancestors = crate::validate::quest_ancestors(c)
            .into_iter()
            .map(|(q, a)| {
                (
                    q.to_string(),
                    a.into_iter().map(str::to_string).collect::<BTreeSet<_>>(),
                )
            })
            .collect();
        let mut prereqs = BTreeMap::new();
        for q in &c.quests.content.quests {
            let direct: BTreeMap<&str, Vec<&str>> = q
                .objectives
                .iter()
                .map(|o| {
                    (
                        o.id().as_str(),
                        o.after().iter().map(|a| a.as_str()).collect(),
                    )
                })
                .collect();
            let mut closed = BTreeMap::new();
            for o in direct.keys() {
                let mut seen = BTreeSet::new();
                let mut stack: Vec<&str> = direct.get(o).cloned().unwrap_or_default();
                while let Some(cur) = stack.pop() {
                    if seen.insert(cur.to_string()) {
                        stack.extend(direct.get(cur).cloned().unwrap_or_default());
                    }
                }
                closed.insert(o.to_string(), seen);
            }
            prereqs.insert(q.id.as_str().to_string(), closed);
        }
        Order {
            campaign: c,
            locs,
            ancestors,
            prereqs,
        }
    }

    /// The JSON pointer of an effect this index holds.
    pub fn path_of(&self, e: &QuestEffect) -> Option<&str> {
        self.locs.get(&key(e)).map(|l| l.path.as_str())
    }

    /// `true` when quest `a` is a strict ancestor of quest `b`.
    fn quest_before(&self, a: &str, b: &str) -> bool {
        self.ancestors.get(b).is_some_and(|anc| anc.contains(a))
    }

    /// `true` when objective `a` must complete before `b` activates (both of
    /// `quest`).
    fn objective_before(&self, quest: &str, a: &str, b: &str) -> bool {
        self.prereqs
            .get(quest)
            .and_then(|m| m.get(b))
            .is_some_and(|s| s.contains(a))
    }

    /// Whether root `a` has certainly fired before root `b` fires.
    fn root_before(&self, a: &Root, b: &Root) -> bool {
        match (a, b) {
            (Root::Ambient, _) | (_, Root::Ambient) => false,
            (Root::QuestComplete { quest: qa }, Root::QuestComplete { quest: qb })
            | (Root::QuestComplete { quest: qa }, Root::Objective { quest: qb, .. }) => {
                self.quest_before(qa, qb)
            }
            (Root::Objective { quest: qa, .. }, Root::QuestComplete { quest: qb }) => {
                qa == qb || self.quest_before(qa, qb)
            }
            (
                Root::Objective {
                    quest: qa,
                    objective: oa,
                },
                Root::Objective {
                    quest: qb,
                    objective: ob,
                },
            ) => self.quest_before(qa, qb) || (qa == qb && self.objective_before(qa, oa, ob)),
        }
    }

    /// Whether root `a` has certainly fired before objective `ob` of quest
    /// `qb` activates.
    fn root_before_activation(&self, a: &Root, qb: &str, ob: &str) -> bool {
        match a {
            Root::Ambient => false,
            Root::QuestComplete { quest } => self.quest_before(quest, qb),
            Root::Objective { quest, objective } => {
                self.quest_before(quest, qb) || (quest == qb && self.objective_before(qb, objective, ob))
            }
        }
    }

    /// Whether objective `oa` of `qa` has certainly activated before root
    /// `b` fires.
    fn activation_before_root(&self, qa: &str, oa: &str, b: &Root) -> bool {
        match b {
            Root::Ambient => false,
            Root::QuestComplete { quest } => quest == qa || self.quest_before(qa, quest),
            Root::Objective { quest, objective } => {
                self.quest_before(qa, quest)
                    || (quest == qa && (objective == oa || self.objective_before(qa, oa, objective)))
            }
        }
    }

    /// **The one ordering rule**: is beat `a` certain to have run, on every
    /// playthrough, by the time beat `b` runs? See the module note for the
    /// model. An effect this index does not hold is ordered with nothing.
    pub fn ordered_before(&self, a: Beat<'_>, b: Beat<'_>) -> bool {
        match (a, b) {
            (Beat::Setup, Beat::Setup) => false,
            (Beat::Setup, _) => true,
            (_, Beat::Setup) => false,
            (Beat::Activation { .. }, Beat::Activation { .. }) => false,
            (Beat::Effect(ea), Beat::Activation { quest, objective }) => {
                let Some(la) = self.locs.get(&key(ea)) else {
                    return false;
                };
                la.certain && self.root_before_activation(&la.root, quest, objective)
            }
            (Beat::Activation { quest, objective }, Beat::Effect(eb)) => {
                let Some(lb) = self.locs.get(&key(eb)) else {
                    return false;
                };
                self.activation_before_root(quest, objective, &lb.root)
            }
            (Beat::Effect(ea), Beat::Effect(eb)) => {
                let (Some(la), Some(lb)) = (self.locs.get(&key(ea)), self.locs.get(&key(eb))) else {
                    return false;
                };
                if !la.certain {
                    return false;
                }
                if la.root_path == lb.root_path {
                    return within_root(&la.chain, &lb.chain);
                }
                self.root_before(&la.root, &lb.root)
            }
        }
    }

    /// Whether `body` (an `npc/`, `actor/` or `assembly/` id) stands in the
    /// world when beat `at` runs: some placing is ordered before it, and no
    /// removal is ordered between that placing and the beat (spec-0110 §5.6).
    pub fn presence(&self, body: &str, at: Beat<'_>) -> Presence {
        let setup = self
            .campaign
            .npcs
            .content
            .npcs
            .iter()
            .any(|n| n.id.as_str() == body && !n.deferred);
        let mut placings: Vec<(Beat<'_>, String)> = Vec::new();
        if setup {
            placings.push((Beat::Setup, "setup".to_string()));
        }
        let mut removals: Vec<(&QuestEffect, String)> = Vec::new();
        crate::effects::for_each_campaign_effect(self.campaign, &mut |path, _site, e| {
            if placing_of(e) == Some(body) {
                placings.push((Beat::Effect(e), path.to_string()));
            }
            if removal_of(e) == Some(body) {
                removals.push((e, path.to_string()));
            }
        });
        let before: Vec<&(Beat<'_>, String)> = placings
            .iter()
            .filter(|(p, _)| self.ordered_before(*p, at))
            .collect();
        if before.is_empty() {
            return Presence::NeverPlaced(placings.into_iter().map(|(_, p)| p).collect());
        }
        let mut first_removed: Option<(String, String)> = None;
        for (p, ppath) in &before {
            let between = removals.iter().find(|(r, _)| {
                self.ordered_before(Beat::Effect(r), at) && self.ordered_before(*p, Beat::Effect(r))
            });
            match between {
                None => return Presence::Present(ppath.clone()),
                Some((_, rpath)) => {
                    first_removed.get_or_insert((ppath.clone(), rpath.clone()));
                }
            }
        }
        let (p, r) = first_removed.unwrap_or_default();
        Presence::Removed(p, r)
    }
}

/// The body an effect puts into the world: `spawn-npc`, `spawn-actor`, or
/// `spawn-assembly`.
pub fn placing_of(e: &QuestEffect) -> Option<&str> {
    match &e.verb {
        Verb::SpawnAssembly { assembly } => Some(assembly.as_str()),
        _ => e.body_entry(),
    }
}

/// The body an effect takes out of the world: `despawn-npc`,
/// `despawn-actor`, `unleash-actor`, or `despawn-assembly`.
pub fn removal_of(e: &QuestEffect) -> Option<&str> {
    match &e.verb {
        Verb::DespawnAssembly { assembly } => Some(assembly.as_str()),
        _ => e.body_exit(),
    }
}

/// Within one root: A (certain) before B.
fn within_root(a: &[Link], b: &[Link]) -> bool {
    // The deepest level at which both sit in the same list.
    let mut k = 0;
    while k < a.len() && k < b.len() && a[k] == b[k] {
        k += 1;
    }
    if k >= a.len() || k >= b.len() {
        // One is the other, or contains it.
        return false;
    }
    // A must sit directly at this level: a beat nested inside an earlier
    // sibling runs when that sibling's own list says, not now.
    if a.len() != k + 1 {
        return false;
    }
    let (la, lb) = (&a[k], &b[k]);
    match (la.step, lb.step) {
        (Some(sa), Some(sb)) if sa != sb => sa < sb,
        _ => la.index < lb.index,
    }
}

#[allow(clippy::too_many_arguments)]
fn index_list(
    list: &[QuestEffect],
    root_path: &str,
    list_path: &str,
    root: &Root,
    prefix: &[Link],
    step: Option<(u32, usize)>,
    certain: bool,
    out: &mut BTreeMap<usize, Loc>,
) {
    for (i, e) in list.iter().enumerate() {
        let mut chain = prefix.to_vec();
        chain.push(Link { index: i, step });
        let path = format!("{list_path}/{i}");
        let here = certain && !gated(e);
        out.insert(
            key(e),
            Loc {
                root_path: root_path.to_string(),
                root: root.clone(),
                chain: chain.clone(),
                path: path.clone(),
                certain: here,
            },
        );
        if let Verb::Sequence { steps } = &e.verb {
            for (s, st) in steps.iter().enumerate() {
                index_list(
                    &st.effects,
                    root_path,
                    &format!("{path}/steps/{s}/effects"),
                    root,
                    &chain,
                    Some((st.at_ticks, s)),
                    here,
                    out,
                );
            }
            continue;
        }
        for (pseg, kseg, inner) in e.nested_effect_lists_labeled() {
            // A reaction bundle fires only if its reaction happens.
            let reaction = matches!(kseg.as_str(), "respawn" | "rest" | "caught");
            index_list(
                inner,
                root_path,
                &format!("{path}/{pseg}"),
                root,
                &chain,
                None,
                here && !reaction,
                out,
            );
        }
    }
}
