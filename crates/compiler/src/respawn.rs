//! **What separates a retry from a soft-lock** (spec-0044) — the evidence
//! `DW0478` accepts.
//!
//! The safe-zone rule asks *"does any hostile's declared perception radius cover
//! the respawn cell?"* and answered it correctly on geometry while claiming
//! something geometry cannot decide: that the retry loop is unwinnable. Whether a
//! loop is winnable is a combat question, and this compiler refuses to simulate
//! combat (ADR-0006). So the red's claim shrinks to what declarations can carry —
//! *nothing the campaign declares separates this respawn from a soft-lock* — and
//! this module holds the three routes by which a campaign supplies that
//! separation:
//!
//! * a **reset**: the fold of the respawn point's own unconditional `on_respawn`
//!   (a bonfire's `on_rest`) leaves the force absent, or standing somewhere else;
//! * an **onset bound**: the force's staging cannot reach the reign at all —
//!   because its gate flags cannot be set in time, because the trigger's bearer is
//!   gone, or because the body is a `NoAI` puppet for every instant compared;
//! * **dominance**: the campaign's own forced critical path already walks the
//!   party into that force, inside the same reign, no farther away and against no
//!   fresher a body.
//!
//! Every route demands a fact vanilla structurally contradicts when the defect is
//! real, and every one falls to a **conservative zero** when the evidence is
//! missing or ambiguous — the pair stays compared and the geometry demanded of it
//! does not move by one block. The kind is computed from the object; there is no
//! field an author writes to claim a credit, which is the vacuity mode that
//! survives every other check (CLAUDE.md).
//!
//! Determinism (ADR-0006): every walk is over the one effect-root enumeration and
//! over slices in declaration order; every map is a `BTreeMap`.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{EffectRootOwner, QuestEffect, TriggerOn, for_each_effect_root};

use crate::nav::World;
use crate::plan::{CheckpointPlan, Plan};

/// The conservative answer whenever a bound does not resolve: *it could be there
/// from the start*. Named because it is the direction every fallback in this
/// module takes, and a fallback that drifted the other way would be a proof that
/// looked away.
const CONSERVATIVE_ZERO: usize = 0;

/// What a bundle is, for ordering. Two effects share a bundle exactly when they
/// are emitted into one function, which vanilla runs in one tick — so **no tick
/// boundary exists between two lines of one bundle**, and nothing polled on the
/// tick (a trigger, a trap) can fire between them. That is the whole content of
/// "emission granularity" (spec-0044 §3) as an ordering fact.
type BundleId = usize;

/// What one emitted effect does, as far as this proof is concerned. Storing the
/// answer rather than the `&QuestEffect` keeps the index owned, which is what lets
/// it be built once and asked many questions.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Act {
    SetFlag(String),
    Stage(String),
    RemoveActor(String),
    Unleash(String),
    MoveActor(String),
    SpawnNpc(String),
    DespawnNpc(String),
    Other,
}

/// Which root fires a bundle — the part of the site this proof reads.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Root {
    /// A quest bundle: the party is forced to fire it, at a known step.
    Forced,
    /// An environment trigger. `bearer` is the NPC whose body the trigger rides
    /// (`strike-npc`) — a trigger vanilla structurally cannot fire without.
    /// `approach` is its declared region, when it watches a place at a range.
    Trigger {
        id: String,
        bearer: Option<String>,
        approach: Option<(String, u32)>,
    },
    /// Every other ambient root — a trap payload, a shortcut unlock, a death
    /// bundle, a shop offer, a dialogue respawn hook. Optional, with no step.
    Ambient,
}

/// One emitted effect, sited.
#[derive(Clone, Debug)]
struct Sited {
    bundle: BundleId,
    /// Emitted line order within the bundle.
    line: usize,
    /// The critical-path step the bundle fires at; [`CONSERVATIVE_ZERO`] for a
    /// root with no beat of its own.
    step: usize,
    /// Every flag that must be set for this line to run — the root's own gate plus
    /// every enclosing effect's `requires_flags` plus its own.
    requires: Vec<String>,
    /// Whether anything on the way down carries a `forbids_flags`. A negatively
    /// gated effect is conditional, and §3 credits no conditional effect.
    negated: bool,
    /// Whether this line sits inside a bundle nothing forces the party to fire —
    /// a `set-checkpoint`'s `on_respawn`, a `bonfire`'s `on_rest`, a
    /// `begin-stealth`'s `on_caught`. Its root is a forced quest bundle, and the
    /// line is still only reached by dying or being caught, so a proof that read
    /// it as forced would credit a removal on a route the party need never take.
    optional: bool,
    root: Root,
    act: Act,
}

impl Sited {
    /// Whether this line runs in **every** state a death can occur in — the only
    /// kind of effect a reset credit or a prefix removal may be read off.
    fn unconditional(&self) -> bool {
        self.requires.is_empty() && !self.negated
    }

    /// Whether the party is FORCED to fire this line: a quest bundle, ungated,
    /// and not inside a death or catch hook.
    fn forced(&self) -> bool {
        self.root == Root::Forced && self.unconditional() && !self.optional
    }
}

/// The perception onset of one force: the earliest critical-path step at which it
/// can both be staged and acquire a target.
#[derive(Clone, Debug)]
pub struct Onset {
    /// The step. [`CONSERVATIVE_ZERO`] whenever anything failed to resolve.
    pub step: usize,
    /// Which bound produced it — `onset`, `flag-bound` or `puppet`, in the
    /// ledger's own vocabulary. Computed from the object, never chosen.
    pub kind: &'static str,
    /// The sentence the ledger prints when this bound is what skips a pair.
    pub reason: String,
}

/// What the post-reset world holds for one force (spec-0044 §3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResetState {
    /// The fold does not touch this force at all.
    Untouched,
    /// An unconditional removal with no later re-stage: the force has no cells.
    Removed(String),
    /// A removal followed by a re-stage: the force stands again, in its re-staged
    /// state, at its own anchor — measured there, and its verdict then passes
    /// through dominance rather than around it.
    ReStaged(String),
    /// The fold touches this force conditionally, or moves it. Nothing is
    /// credited: the post-reset world must hold in every state a death can occur
    /// in.
    Ambiguous(String),
}

/// A credit: why this pair is not evidence of a soft-lock.
#[derive(Clone, Debug)]
pub struct Credit {
    /// `reset` or `dominated` — computed from the object.
    pub kind: &'static str,
    /// The sentence the ledger prints.
    pub reason: String,
    /// The post-reset state, in words.
    pub state: String,
}

/// A skip: why this pair was never compared.
#[derive(Clone, Debug)]
pub struct Skip {
    /// `reign`, `onset`, `flag-bound`, `bearer-bound` or `puppet`.
    pub kind: &'static str,
    pub reason: String,
}

/// The evidence index: built once per build, asked once per pair.
pub struct Evidence {
    sited: Vec<Sited>,
    /// Force id → its perception onset.
    onsets: BTreeMap<String, Onset>,
    /// The bundle each checkpoint's own `set-checkpoint` / `bonfire` line sits in,
    /// by `CheckpointPlan::index`. `None` when the line could not be located
    /// unambiguously — the conservative direction, which costs only the
    /// same-bundle reason.
    checkpoint_bundle: BTreeMap<usize, BundleId>,
    /// The routed critical path: per destination step, every cell the party walks
    /// to reach it (including the endpoints).
    route: Vec<(usize, Vec<[i32; 3]>)>,
}

impl Evidence {
    /// Build the index. `world` is the assembled, edited world the nav proofs
    /// route over, so the path this reads is the path they proved.
    pub fn build(plan: &Plan, world: &World) -> Self {
        let sited = site_effects(plan);
        let flags = flag_bounds(&sited);
        let route = crate::nav::critical_route_cells(plan, world);
        let actors: BTreeSet<String> = plan
            .campaign
            .quests
            .content
            .actors
            .iter()
            .map(|a| a.id.as_str().to_string())
            .collect();
        let anchors = |name: &str| crate::plan::point_any(&plan.anchors, name);
        let onsets = perception_onsets(&actors, &anchors, &sited, &flags, &route);
        let checkpoint_bundle = locate_checkpoints(plan, &sited);
        Self {
            sited,
            onsets,
            checkpoint_bundle,
            route,
        }
    }

    /// The perception onset of a force — [`CONSERVATIVE_ZERO`] for anything this
    /// index has no staging beat for.
    pub fn onset(&self, force: &str) -> Onset {
        self.onsets.get(force).cloned().unwrap_or(Onset {
            step: CONSERVATIVE_ZERO,
            kind: "onset",
            reason: format!(
                "`{force}` is staged from a root with no beat of its own, so it is rooted at \
                 critical-path step 0"
            ),
        })
    }

    /// [`reset_state`], as a method, for a caller that already holds the index.
    pub fn reset_state(&self, cp: &CheckpointPlan, force: &str) -> ResetState {
        reset_state(cp, force)
    }
}

/// The fold of a respawn point's own unconditional `on_respawn` (a bonfire's
/// `on_rest`) over one force (spec-0044 §3).
///
/// Free rather than a method because it reads nothing but the bundle: the
/// post-reset world is a fact about the emitted lines of ONE function, and
/// nothing about the rest of the campaign can change it.
pub(crate) fn reset_state(cp: &CheckpointPlan, force: &str) -> ResetState {
    let mut state = ResetState::Untouched;
    let mut conditional_touch: Option<String> = None;
    walk_bundle(&cp.on_respawn, &mut |eff, gated| {
        let act = act_of(eff);
        let touches = match &act {
            Act::Stage(id) | Act::RemoveActor(id) | Act::Unleash(id) | Act::MoveActor(id) => {
                id == force
            }
            _ => false,
        };
        if !touches {
            return;
        }
        if gated {
            conditional_touch.get_or_insert_with(|| {
                format!(
                    "`{}`'s `{}` bundle touches `{force}` only behind a flag gate, and the \
                     post-reset world must hold in every state a death can occur in — no \
                     credit",
                    cp.anchor,
                    if cp.rest { "on_rest" } else { "on_respawn" }
                )
            });
            return;
        }
        match act {
            Act::RemoveActor(_) => {
                state = ResetState::Removed(format!(
                    "`{}`'s own `{}` bundle unconditionally removes `{force}` \
                     (`despawn-actor`), so the force has no cells in the world the reset \
                     leaves",
                    cp.anchor,
                    if cp.rest { "on_rest" } else { "on_respawn" }
                ));
            }
            Act::Stage(_) => {
                state = ResetState::ReStaged(format!(
                    "`{}`'s own `{}` bundle re-stages `{force}` (`spawn-actor`) at its own \
                     anchor, so the force is measured there, in its re-staged state",
                    cp.anchor,
                    if cp.rest { "on_rest" } else { "on_respawn" }
                ));
            }
            Act::Unleash(_) => { /* an unleash summons nothing; it changes no cell */ }
            Act::MoveActor(_) => {
                state = ResetState::Ambiguous(format!(
                    "`{}`'s reset bundle walks `{force}` somewhere else (`move-actor`); this \
                     proof measures seated cells, so nothing is credited",
                    cp.anchor
                ));
            }
            _ => {}
        }
    });
    if let Some(why) = conditional_touch {
        return ResetState::Ambiguous(why);
    }
    state
}

impl Evidence {
    /// The **bearer bound** (spec-0044 §4a): a trigger keyed to an entity cannot
    /// fire without that entity, so a force staged only from such triggers cannot
    /// be staged once every bearer is unconditionally gone.
    ///
    /// The whole obligation must hold, and every part of it is read off emitted
    /// declarations: every staging trigger is entity-keyed; every bearer is
    /// unconditionally removed by a **forced** bundle at or before the seat; no
    /// bundle anywhere re-stages a bearer at or after that removal; and no
    /// instance staged before the reign can survive into it. Where any part
    /// cannot be established the pair is compared.
    pub fn bearer_bound(&self, cp: &CheckpointPlan, force: &str) -> Option<Skip> {
        let stagings: Vec<&Sited> = self
            .sited
            .iter()
            .filter(|s| matches!(&s.act, Act::Stage(id) if id == force))
            .collect();
        if stagings.is_empty() {
            return None;
        }
        let mut bearers: BTreeSet<String> = BTreeSet::new();
        for s in &stagings {
            match &s.root {
                Root::Trigger {
                    bearer: Some(npc), ..
                } => {
                    bearers.insert(npc.clone());
                }
                // A staging from anywhere else needs no entity to fire it.
                _ => return None,
            }
        }
        let seat = self.checkpoint_bundle.get(&cp.index).copied();
        let mut removals: Vec<String> = Vec::new();
        for npc in &bearers {
            let removal = self
                .sited
                .iter()
                .filter(|s| {
                    matches!(&s.act, Act::DespawnNpc(id) if id == npc)
                        && s.forced()
                        && (s.step < cp.fire_step || Some(s.bundle) == seat)
                })
                .min_by_key(|s| (s.step, s.line))?;
            // Nothing may put the bearer back at or after that removal — on any
            // route, including a reset bundle.
            let restaged = self
                .sited
                .iter()
                .any(|s| matches!(&s.act, Act::SpawnNpc(id) if id == npc) && !precedes(s, removal));
            if restaged {
                return None;
            }
            removals.push(format!(
                "`{npc}` is unconditionally removed at critical-path step {} by a bundle the \
                 party is forced to fire, and no bundle anywhere stages it again",
                removal.step
            ));
        }
        // Route closure: an instance staged BEFORE the reign must not survive into
        // it. Either the force cannot be staged before the reign at all, or every
        // reign that precedes this one folds it away on death AND a forced bundle
        // in the prefix removes it on the surviving route.
        let onset = self.onset(force).step;
        if onset < cp.fire_step && !self.prefix_removes(cp, force) {
            return None;
        }
        Some(Skip {
            kind: "bearer-bound",
            reason: format!(
                "every trigger that stages `{force}` rides an NPC's own body (`strike-npc`), and \
                 vanilla cannot fire an entity-keyed trigger with no entity: {}. No instance \
                 staged before `{}`'s reign survives into it, so nothing can put this force in \
                 the world while this seat governs",
                removals.join("; "),
                cp.anchor
            ),
        })
    }

    /// Whether the **forced** prefix up to and including the seating bundle
    /// unconditionally removes a force — the surviving-route half of §4a's
    /// closure.
    fn prefix_removes(&self, cp: &CheckpointPlan, force: &str) -> bool {
        let seat = self.checkpoint_bundle.get(&cp.index).copied();
        self.sited.iter().any(|s| {
            matches!(&s.act, Act::RemoveActor(id) if id == force)
                && s.forced()
                && (s.step < cp.fire_step || Some(s.bundle) == seat)
        })
    }

    /// **Dominance** (spec-0044 §6): the campaign's own forced critical path
    /// already walks the party into this force, inside this seat's reign, at a
    /// distance the seat does not better, against a body no fresher.
    ///
    /// `cells` are the force's **stationary** cells only — a lane wave's smeared
    /// march corridor never dominates, because the corridor is every cell the
    /// squad sweeps over time and the path crossing it is not a proven meeting.
    /// `seat_distance` is what the seat itself measures to those cells.
    pub fn dominance(
        &self,
        cp: &CheckpointPlan,
        reign_end: Option<usize>,
        force: &str,
        cells: &[[i32; 3]],
        seat_distance: f64,
    ) -> Option<Credit> {
        if cells.is_empty() {
            return None;
        }
        let onset = self.onset(force).step;
        let mut best: Option<(usize, [i32; 3], f64)> = None;
        for (step, walked) in &self.route {
            if *step <= cp.fire_step || *step < onset {
                continue;
            }
            if reign_end.is_some_and(|e| *step >= e) {
                continue;
            }
            for c in walked {
                let d = cells
                    .iter()
                    .map(|k| distance(*c, *k))
                    .fold(f64::INFINITY, f64::min);
                if d <= seat_distance && best.as_ref().is_none_or(|(_, _, b)| d < *b) {
                    best = Some((*step, *c, d));
                }
            }
        }
        let (step, cell, d) = best?;
        Some(Credit {
            kind: "dominated",
            reason: format!(
                "the campaign's own forced critical path already meets `{force}` inside this \
                 seat's reign: reaching critical-path step {step} walks the party across \
                 {cell:?}, {d:.2} blocks from that force's stationary cells, while the seat \
                 stands {seat_distance:.2} blocks from them. The retry delivers an encounter \
                 the path already delivers no-more-gently — same body, no closer, and no \
                 fresher (the meeting is at or after the force's own perception onset, step \
                 {onset}). A dominated respawn can only be a soft-lock if that forced beat is \
                 unwinnable, which is the campaign being uncompletable — refused by the machine \
                 playthrough on evidence no defect can supply (spec-0023)",
            ),
            state: format!("measured at the force's stationary cells; nearest {cell:?}"),
        })
    }
}

/// Whether `a` is provably at or before `b` — a fact about emission order, never a
/// guess. Same bundle: line order decides. Different bundles: only the step
/// decides, and equal steps in different bundles are not ordered at all.
fn precedes(a: &Sited, b: &Sited) -> bool {
    if a.bundle == b.bundle {
        return a.line < b.line;
    }
    a.step < b.step
}

/// Euclidean distance between two cells, in blocks.
fn distance(a: [i32; 3], b: [i32; 3]) -> f64 {
    (0..3)
        .map(|i| f64::from(a[i] - b[i]).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Every emitted effect, sited at emission granularity, over the one effect-root
/// enumeration (`delvewright_dsl::for_each_effect_root`) — so a new root cannot
/// leave a staging beat invisible here.
fn site_effects(plan: &Plan) -> Vec<Sited> {
    let mut out: Vec<Sited> = Vec::new();
    let mut bundle = 0usize;
    for_each_effect_root(plan.campaign, &mut |site, list| {
        let (root, step, gate) = match &site.owner {
            EffectRootOwner::ObjectiveComplete { objective, .. } => (
                Root::Forced,
                plan.objective_steps
                    .get(*objective)
                    .copied()
                    .unwrap_or(CONSERVATIVE_ZERO),
                Vec::new(),
            ),
            EffectRootOwner::QuestComplete { quest } => (
                Root::Forced,
                crate::plan::quest_complete_step(quest, &plan.objective_steps),
                Vec::new(),
            ),
            EffectRootOwner::Trigger(t) => (
                Root::Trigger {
                    id: t.id.as_str().to_string(),
                    bearer: t.on.npc_target().map(|n| n.as_str().to_string()),
                    approach: match (&t.on, t.at_anchor()) {
                        (TriggerOn::Approach { range }, Some(at)) => Some((at.to_string(), *range)),
                        _ => None,
                    },
                },
                CONSERVATIVE_ZERO,
                t.requires_flags
                    .iter()
                    .map(|f| f.as_str().to_string())
                    .collect(),
            ),
            EffectRootOwner::TrapPayload(t) => (
                Root::Ambient,
                CONSERVATIVE_ZERO,
                t.requires_flags
                    .iter()
                    .map(|f| f.as_str().to_string())
                    .collect(),
            ),
            EffectRootOwner::DialogueRespawn
            | EffectRootOwner::ShortcutUnlock(_)
            | EffectRootOwner::OnDeath
            | EffectRootOwner::ShopOffer(_) => (Root::Ambient, CONSERVATIVE_ZERO, Vec::new()),
        };
        let id = bundle;
        bundle += 1;
        let mut line = 0usize;
        let mut push = |eff: &QuestEffect, requires: Vec<String>, negated: bool, optional: bool| {
            out.push(Sited {
                bundle: id,
                line,
                step,
                requires,
                negated,
                optional,
                root: root.clone(),
                act: act_of(eff),
            });
            line += 1;
        };
        deep(list, &gate, false, false, &mut push);
    });
    out
}

/// Descend a bundle in emitted line order, accumulating every enclosing gate.
/// What [`deep`] hands each visited line: the effect, the gates it inherits,
/// whether anything on the way down forbids a flag, and whether it sits inside a
/// bundle only a death or a catch reaches.
type LineVisitor<'a> = dyn FnMut(&QuestEffect, Vec<String>, bool, bool) + 'a;

fn deep(
    list: &[QuestEffect],
    inherited: &[String],
    negated: bool,
    optional: bool,
    push: &mut LineVisitor<'_>,
) {
    for eff in list {
        let mut requires: Vec<String> = inherited.to_vec();
        requires.extend(eff.requires_flags().iter().map(|f| f.as_str().to_string()));
        let negated = negated || !eff.forbids_flags().is_empty();
        push(eff, requires.clone(), negated, optional);
        for (pseg, _, inner) in eff.nested_effect_lists_labeled() {
            // `on_respawn` / `on_rest` / `on_caught` are reached by dying or by
            // being caught. Their root is a forced quest bundle and the lines
            // inside are not forced, which is exactly the reading that would
            // credit a removal on a route the party need never take.
            let optional =
                optional || matches!(pseg.as_str(), "on_respawn" | "on_rest" | "on_caught");
            deep(inner, &requires, negated, optional, push);
        }
    }
}

/// Walk one bundle in emitted line order, telling the visitor whether the line is
/// behind any gate at all.
fn walk_bundle(list: &[QuestEffect], f: &mut dyn FnMut(&QuestEffect, bool)) {
    fn go(list: &[QuestEffect], gated: bool, f: &mut dyn FnMut(&QuestEffect, bool)) {
        for eff in list {
            let gated =
                gated || !eff.requires_flags().is_empty() || !eff.forbids_flags().is_empty();
            f(eff, gated);
            for (_, _, inner) in eff.nested_effect_lists_labeled() {
                go(inner, gated, f);
            }
        }
    }
    go(list, false, f);
}

/// What one effect does, as this proof reads it.
fn act_of(eff: &QuestEffect) -> Act {
    match eff {
        QuestEffect::SetFlag { flag, .. } => Act::SetFlag(flag.as_str().to_string()),
        QuestEffect::SpawnWave { wave, .. } => Act::Stage(wave.as_str().to_string()),
        QuestEffect::SpawnActor { actor, .. } => Act::Stage(actor.as_str().to_string()),
        QuestEffect::DespawnActor { actor, .. } => Act::RemoveActor(actor.as_str().to_string()),
        QuestEffect::UnleashActor { actor, .. } => Act::Unleash(actor.as_str().to_string()),
        QuestEffect::MoveActor { actor, .. } => Act::MoveActor(actor.as_str().to_string()),
        QuestEffect::SpawnNpc { npc, .. } => Act::SpawnNpc(npc.as_str().to_string()),
        QuestEffect::DespawnNpc { npc, .. } => Act::DespawnNpc(npc.as_str().to_string()),
        _ => Act::Other,
    }
}

/// **The flag bound** (spec-0044 §4a): the earliest critical-path step at which
/// each flag can be set, resolved recursively over its producers' own gates.
///
/// A cycle or an unresolvable producer falls to [`CONSERVATIVE_ZERO`] — a force
/// that meets a reign cannot supply a gate that provably kept it out of the
/// reign, so the fallback must be the direction that compares.
fn flag_bounds(sited: &[Sited]) -> BTreeMap<String, usize> {
    let mut producers: BTreeMap<String, Vec<(usize, Vec<String>)>> = BTreeMap::new();
    for s in sited {
        if let Act::SetFlag(flag) = &s.act {
            producers
                .entry(flag.clone())
                .or_default()
                .push((s.step, s.requires.clone()));
        }
    }
    let flags: Vec<String> = producers.keys().cloned().collect();
    flags
        .iter()
        .map(|f| {
            let mut seen = BTreeSet::new();
            (
                f.clone(),
                resolve_flag(f, &producers, &mut seen).unwrap_or(CONSERVATIVE_ZERO),
            )
        })
        .collect()
}

/// The earliest step a flag can be set, or `None` when nothing establishes an
/// ordering at all — a cycle, or a gate on a flag no bundle produces.
///
/// `None` propagates: a producer whose own gate is unresolvable is a producer
/// nothing bounds, so the flag it sets is unbounded too. Deliberately not
/// memoised — the answer depends on the path taken to reach it, and a cached
/// value from one path is a wrong answer on another. Flag counts are tens, so the
/// cost is nothing and the correctness is free.
fn resolve_flag(
    flag: &str,
    producers: &BTreeMap<String, Vec<(usize, Vec<String>)>>,
    seen: &mut BTreeSet<String>,
) -> Option<usize> {
    if !seen.insert(flag.to_string()) {
        return None;
    }
    let out = (|| {
        let sites = producers.get(flag)?;
        let mut best = usize::MAX;
        for (step, gate) in sites {
            let mut when = *step;
            for g in gate {
                when = when.max(resolve_flag(g, producers, seen)?);
            }
            best = best.min(when);
        }
        (best != usize::MAX).then_some(best)
    })();
    seen.remove(flag);
    out
}

/// The perception onset of every force: the earliest step at which it is both
/// **staged** and **able to acquire a target**.
///
/// The staging half is the flag-bounded earliest staging beat. The second half is
/// spec-0044 §4b — a puppet is not a perceiver. An actor's staged body is
/// `NoAI:1b` by construction; `unleash-actor` is what replaces it with a real-AI
/// twin. So for an actor the onset is `max(staging, unleash bound)`, and the
/// unleash bound resolves by the same machinery: a step-rooted unleash at its
/// step, a flag-gated one at its flag bound, a proximity-triggered one at the
/// earliest critical-path entry into the trigger's own declared region, and
/// [`CONSERVATIVE_ZERO`] whenever nothing resolves — which makes the whole bound
/// strictly narrowing, since `max(s, 0)` is the answer this proof gave before.
fn perception_onsets(
    actors: &BTreeSet<String>,
    anchors: &dyn Fn(&str) -> Option<[i32; 3]>,
    sited: &[Sited],
    flags: &BTreeMap<String, usize>,
    route: &[(usize, Vec<[i32; 3]>)],
) -> BTreeMap<String, Onset> {
    let gate_step = |requires: &[String]| -> usize {
        requires
            .iter()
            .map(|f| flags.get(f).copied().unwrap_or(CONSERVATIVE_ZERO))
            .max()
            .unwrap_or(CONSERVATIVE_ZERO)
    };
    let mut out: BTreeMap<String, Onset> = BTreeMap::new();
    // Staging.
    for s in sited {
        let Act::Stage(id) = &s.act else { continue };
        let when = s.step.max(gate_step(&s.requires));
        let (kind, reason) = if when > s.step {
            (
                "flag-bound",
                format!(
                    "`{id}` is staged only from a bundle gated on {} — flags nothing can set \
                     before critical-path step {when}, resolved over each flag's own producers \
                     at emission order",
                    s.requires
                        .iter()
                        .map(|f| format!("`{f}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        } else {
            (
                "onset",
                format!("`{id}` is first staged at critical-path step {when}"),
            )
        };
        match out.get(id) {
            Some(prev) if prev.step <= when => {}
            _ => {
                out.insert(
                    id.clone(),
                    Onset {
                        step: when,
                        kind,
                        reason,
                    },
                );
            }
        }
    }
    // The puppet bound: an actor perceives nothing until it is unleashed.
    let mut unleash: BTreeMap<String, usize> = BTreeMap::new();
    for s in sited {
        let Act::Unleash(id) = &s.act else { continue };
        let mut when = s.step.max(gate_step(&s.requires));
        if let Root::Trigger {
            approach: Some((anchor, range)),
            ..
        } = &s.root
        {
            when = when.max(approach_entry(anchors, route, anchor, *range));
        }
        let slot = unleash.entry(id.clone()).or_insert(when);
        *slot = (*slot).min(when);
    }
    for id in actors {
        let Some(base) = out.get(id).cloned() else {
            continue;
        };
        let u = unleash.get(id).copied().unwrap_or(CONSERVATIVE_ZERO);
        if u > base.step {
            out.insert(
                id.clone(),
                Onset {
                    step: u,
                    kind: "puppet",
                    reason: format!(
                        "`{id}` is staged at critical-path step {} as a `NoAI` puppet — the \
                         emitted body carries `NoAI:1b`, and vanilla gives a `NoAI` mob no \
                         target acquisition at all. Nothing can unleash it before step {u}, so \
                         it perceives nothing until then",
                        base.step
                    ),
                },
            );
        }
    }
    out
}

/// The earliest critical-path step at which the party's own routed walk enters a
/// declared approach region. [`CONSERVATIVE_ZERO`] when the anchor does not
/// resolve or nothing enters it — the direction that compares.
fn approach_entry(
    anchors: &dyn Fn(&str) -> Option<[i32; 3]>,
    route: &[(usize, Vec<[i32; 3]>)],
    anchor: &str,
    range: u32,
) -> usize {
    let Some(at) = anchors(anchor) else {
        return CONSERVATIVE_ZERO;
    };
    let r = f64::from(range);
    route
        .iter()
        .filter(|(_, cells)| cells.iter().any(|c| distance(*c, at) <= r))
        .map(|(step, _)| *step)
        .min()
        .unwrap_or(CONSERVATIVE_ZERO)
}

/// Which bundle each checkpoint's own seating line sits in, keyed by
/// `CheckpointPlan::index`.
///
/// The match is by anchor and by kind, and an ambiguous anchor is left unmapped:
/// the only thing the mapping buys is the same-bundle ordering fact, and a wrong
/// bundle would be a wrong ordering fact.
fn locate_checkpoints(plan: &Plan, sited: &[Sited]) -> BTreeMap<usize, BundleId> {
    let mut by_anchor: BTreeMap<String, Vec<BundleId>> = BTreeMap::new();
    let mut bundle = 0usize;
    for_each_effect_root(plan.campaign, &mut |_, list| {
        let id = bundle;
        bundle += 1;
        walk_bundle(list, &mut |eff, _| {
            let anchor = match eff {
                QuestEffect::SetCheckpoint { anchor, .. } | QuestEffect::Bonfire { anchor, .. } => {
                    anchor.as_str().to_string()
                }
                _ => return,
            };
            by_anchor.entry(anchor).or_default().push(id);
        });
    });
    let _ = sited;
    let mut out = BTreeMap::new();
    for cp in &plan.checkpoints {
        if let Some(ids) = by_anchor.get(&cp.anchor)
            && ids.len() == 1
        {
            out.insert(cp.index, ids[0]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use delvewright_dsl::{ActorId, AnchorId, DespawnStyle, FlagId, NpcId};

    fn actor(id: &str) -> ActorId {
        ActorId(id.to_string())
    }

    fn despawn(id: &str) -> QuestEffect {
        QuestEffect::DespawnActor {
            actor: actor(id),
            style: DespawnStyle::Vanish,
            happening: None,
        }
    }

    fn spawn(id: &str) -> QuestEffect {
        QuestEffect::SpawnActor {
            actor: actor(id),
            happening: None,
        }
    }

    /// A `move-npc` gated on a flag, carrying `inner` in its `on_arrive` — the one
    /// shape in the DSL that makes an actor-staging verb conditional, since
    /// `despawn-actor` itself is world-global staging and carries no per-effect
    /// gate of its own.
    fn gated(flag: &str, inner: Vec<QuestEffect>) -> QuestEffect {
        QuestEffect::MoveNpc {
            npc: NpcId("npc/keeper".into()),
            to_anchor: AnchorId("anchor/door".into()),
            speed: None,
            on_arrive: inner,
            requires_flags: vec![FlagId(flag.into())],
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
            happening: None,
        }
    }

    fn checkpoint(on_respawn: Vec<QuestEffect>) -> CheckpointPlan {
        CheckpointPlan {
            index: 0,
            anchor: "anchor/alcove".into(),
            pos: [0, 64, 0],
            on_respawn,
            fire_step: 3,
            rest: false,
            prompt: String::new(),
            rest_label: String::new(),
            save_label: String::new(),
        }
    }

    /// **The reset credit** (spec-0044 §3): an unconditional `despawn-actor` with
    /// no later re-stage leaves the force with no cells in the world the reset
    /// leaves — the fact the defect ("F perceives the arrival") contradicts.
    #[test]
    fn an_unconditional_despawn_in_the_reset_bundle_removes_the_force() {
        let cp = checkpoint(vec![despawn("actor/warden")]);
        let ResetState::Removed(why) = reset_state(&cp, "actor/warden") else {
            panic!(
                "an unconditional despawn is a removal: {:?}",
                reset_state(&cp, "actor/warden")
            );
        };
        assert!(
            why.contains("despawn-actor") && why.contains("actor/warden"),
            "{why}"
        );
    }

    /// The same bundle, one line different: a despawn followed by a re-stage puts
    /// the body back, so the pair is MEASURED at the re-staged cells rather than
    /// credited. Line order inside one bundle is what decides it.
    #[test]
    fn a_despawn_then_respawn_is_a_re_stage_and_the_order_is_what_decides() {
        let restage = checkpoint(vec![despawn("actor/warden"), spawn("actor/warden")]);
        assert!(
            matches!(
                reset_state(&restage, "actor/warden"),
                ResetState::ReStaged(_)
            ),
            "despawn then spawn is a re-stage"
        );
        let removed = checkpoint(vec![spawn("actor/warden"), despawn("actor/warden")]);
        assert!(
            matches!(
                reset_state(&removed, "actor/warden"),
                ResetState::Removed(_)
            ),
            "the same two lines the other way round is a removal — emission order decides"
        );
    }

    /// **The credit's red side.** A conditional removal is never credited: the
    /// post-reset world must hold in EVERY state a death can occur in, and "true
    /// in the author's head, false in some flag state" has no route in.
    #[test]
    fn a_gated_removal_is_never_credited() {
        let cp = checkpoint(vec![gated("flag/opened", vec![despawn("actor/warden")])]);
        let ResetState::Ambiguous(why) = reset_state(&cp, "actor/warden") else {
            panic!("a flag-gated removal is not a removal");
        };
        assert!(why.contains("flag gate"), "{why}");
        // …and a bundle that never mentions the force at all is untouched, which
        // is a third answer rather than a quiet third spelling of "removed".
        assert_eq!(
            reset_state(&checkpoint(vec![despawn("actor/other")]), "actor/warden"),
            ResetState::Untouched
        );
    }

    fn sited(bundle: usize, line: usize, step: usize, requires: &[&str], act: Act) -> Sited {
        Sited {
            bundle,
            line,
            step,
            requires: requires.iter().map(|s| (*s).to_string()).collect(),
            negated: false,
            optional: false,
            root: Root::Forced,
            act,
        }
    }

    /// **The flag bound** (spec-0044 §4a), in the direction that narrows: a flag
    /// set only by a bundle at step 6 cannot be set before step 6, and a flag
    /// whose own producer is gated on it inherits that bound.
    #[test]
    fn a_flag_is_bounded_by_its_producers_and_by_their_gates() {
        let sited = vec![
            sited(0, 0, 6, &[], Act::SetFlag("flag/sealed".into())),
            sited(
                1,
                0,
                2,
                &["flag/sealed"],
                Act::SetFlag("flag/roused".into()),
            ),
        ];
        let flags = flag_bounds(&sited);
        assert_eq!(flags["flag/sealed"], 6);
        assert_eq!(
            flags["flag/roused"], 6,
            "a producer gated on a flag cannot fire before that flag's own bound"
        );
    }

    /// …and the conservative direction, which is what keeps the bound honest: a
    /// cycle resolves to 0 and the pair is compared. A force that meets a reign
    /// cannot supply a gate that provably kept it out of the reign.
    #[test]
    fn a_flag_cycle_falls_to_the_conservative_zero() {
        let sited = vec![
            sited(0, 0, 4, &["flag/b"], Act::SetFlag("flag/a".into())),
            sited(1, 0, 4, &["flag/a"], Act::SetFlag("flag/b".into())),
        ];
        let flags = flag_bounds(&sited);
        assert_eq!(flags["flag/a"], CONSERVATIVE_ZERO);
        assert_eq!(flags["flag/b"], CONSERVATIVE_ZERO);
    }

    fn no_anchors(_: &str) -> Option<[i32; 3]> {
        None
    }

    /// **The puppet bound** (spec-0044 §4b): an actor's staged body is `NoAI:1b`,
    /// and vanilla gives a `NoAI` mob no target acquisition at all, so the force's
    /// perception onset is the beat that unleashes it — not the beat that stages
    /// it.
    #[test]
    fn a_puppet_perceives_nothing_until_it_is_unleashed() {
        let actors: BTreeSet<String> = ["actor/warden".to_string()].into_iter().collect();
        let sited = vec![
            sited(0, 0, 1, &[], Act::Stage("actor/warden".into())),
            sited(1, 0, 9, &[], Act::Unleash("actor/warden".into())),
        ];
        let onsets = perception_onsets(&actors, &no_anchors, &sited, &BTreeMap::new(), &[]);
        assert_eq!(onsets["actor/warden"].step, 9);
        assert_eq!(onsets["actor/warden"].kind, "puppet");
        assert!(onsets["actor/warden"].reason.contains("NoAI"));
    }

    /// …and its red side. An unleash nothing resolves falls to 0, so the bound
    /// never widens the window: `max(staging, 0)` is the answer this proof gave
    /// before the bound existed.
    #[test]
    fn an_unresolvable_unleash_leaves_the_staging_onset_alone() {
        let actors: BTreeSet<String> = ["actor/warden".to_string()].into_iter().collect();
        let sited = vec![
            sited(0, 0, 1, &[], Act::Stage("actor/warden".into())),
            // A trigger-rooted unleash with no gate and no approach region: step 0.
            Sited {
                root: Root::Trigger {
                    id: "trigger/anything".into(),
                    bearer: None,
                    approach: None,
                },
                ..sited(1, 0, 0, &[], Act::Unleash("actor/warden".into()))
            },
        ];
        let onsets = perception_onsets(&actors, &no_anchors, &sited, &BTreeMap::new(), &[]);
        assert_eq!(onsets["actor/warden"].step, 1, "unchanged by the bound");
        assert_ne!(onsets["actor/warden"].kind, "puppet");
    }

    fn evidence(sited: Vec<Sited>, route: Vec<(usize, Vec<[i32; 3]>)>) -> Evidence {
        let actors: BTreeSet<String> = ["actor/warden".to_string()].into_iter().collect();
        let flags = flag_bounds(&sited);
        let onsets = perception_onsets(&actors, &no_anchors, &sited, &flags, &route);
        Evidence {
            sited,
            onsets,
            checkpoint_bundle: [(0usize, 7usize)].into_iter().collect(),
            route,
        }
    }

    /// A staging that rides an NPC's own body, gated so the force cannot be in
    /// the world before the seat is armed (`fire_step` 3). Both halves matter:
    /// the bound is about what can be staged DURING the reign, and a force that
    /// could already be standing there needs the route-closure half instead.
    fn strike_npc(bundle: usize, line: usize, act: Act) -> Sited {
        Sited {
            root: Root::Trigger {
                id: "trigger/wake".into(),
                bearer: Some("npc/host".into()),
                approach: None,
            },
            ..sited(bundle, line, 0, &["flag/late"], act)
        }
    }

    /// The producer that puts `flag/late` at step 4 — after the seat.
    fn late_flag() -> Sited {
        sited(2, 0, 4, &[], Act::SetFlag("flag/late".into()))
    }

    /// **The bearer bound** (spec-0044 §4a): a trigger keyed to an entity cannot
    /// fire without that entity. Every staging trigger rides `npc/host`, a forced
    /// bundle removes it unconditionally inside the seating bundle, nothing
    /// stages it again, and the force's own onset is not before the reign — so
    /// nothing can put this force in the world while the seat governs.
    #[test]
    fn a_gone_bearer_closes_the_staging_window() {
        let ev = evidence(
            vec![
                strike_npc(1, 0, Act::Stage("actor/warden".into())),
                late_flag(),
                sited(7, 2, 3, &[], Act::DespawnNpc("npc/host".into())),
            ],
            Vec::new(),
        );
        let cp = checkpoint(Vec::new());
        let skip = ev
            .bearer_bound(&cp, "actor/warden")
            .expect("the bearer is gone and every route is closed");
        assert_eq!(skip.kind, "bearer-bound");
        assert!(skip.reason.contains("npc/host") && skip.reason.contains("strike-npc"));
    }

    /// …and three red sides, each of which must leave the pair COMPARED.
    #[test]
    fn the_bearer_bound_refuses_every_route_it_cannot_close() {
        let cp = checkpoint(Vec::new());
        // (a) the bearer's removal is itself conditional.
        let conditional = evidence(
            vec![
                strike_npc(1, 0, Act::Stage("actor/warden".into())),
                late_flag(),
                sited(7, 2, 3, &["flag/x"], Act::DespawnNpc("npc/host".into())),
            ],
            Vec::new(),
        );
        assert!(conditional.bearer_bound(&cp, "actor/warden").is_none());
        // (b) something stages the bearer again after the removal.
        let restaged = evidence(
            vec![
                strike_npc(1, 0, Act::Stage("actor/warden".into())),
                late_flag(),
                sited(7, 2, 3, &[], Act::DespawnNpc("npc/host".into())),
                sited(9, 0, 5, &[], Act::SpawnNpc("npc/host".into())),
            ],
            Vec::new(),
        );
        assert!(restaged.bearer_bound(&cp, "actor/warden").is_none());
        // (c) an instance staged BEFORE the reign that no fold removes. The
        //     staging trigger is ungated, so the force's onset is step 0 —
        //     before the seat — and nothing in the forced prefix removes it.
        //     This is the route-closure half, and it is the one a targeted
        //     reading of the rule would miss.
        let survives = evidence(
            vec![
                // No flag gate: the force can already be standing there when the
                // seat is armed, and no forced bundle takes it off the board.
                Sited {
                    root: Root::Trigger {
                        id: "trigger/wake".into(),
                        bearer: Some("npc/host".into()),
                        approach: None,
                    },
                    ..sited(1, 0, 0, &[], Act::Stage("actor/warden".into()))
                },
                sited(7, 2, 3, &[], Act::DespawnNpc("npc/host".into())),
            ],
            Vec::new(),
        );
        assert!(
            survives.onset("actor/warden").step < cp.fire_step,
            "the fixture must actually exercise the surviving-instance route"
        );
        assert!(
            survives.bearer_bound(&cp, "actor/warden").is_none(),
            "a gone bearer closes future stagings, never an instance already standing"
        );
        // …and the same evidence WITH a forced unconditional removal in the
        // prefix is what closes that route.
        let closed = evidence(
            vec![
                Sited {
                    root: Root::Trigger {
                        id: "trigger/wake".into(),
                        bearer: Some("npc/host".into()),
                        approach: None,
                    },
                    ..sited(1, 0, 0, &[], Act::Stage("actor/warden".into()))
                },
                sited(7, 2, 3, &[], Act::DespawnNpc("npc/host".into())),
                sited(7, 3, 3, &[], Act::RemoveActor("actor/warden".into())),
            ],
            Vec::new(),
        );
        assert!(closed.bearer_bound(&cp, "actor/warden").is_some());
    }

    /// **A line only a death reaches is not part of the forced prefix.** A
    /// `set-checkpoint`'s `on_respawn` hangs off a quest bundle the party must
    /// fire, so its root is forced and its lines are not: the party reaches them
    /// by dying. Reading one as forced credits a removal on a route nobody has to
    /// take — which is what turned the released island's own reset bundle into a
    /// bearer-bound skip for a body that can still be standing there.
    #[test]
    fn a_removal_inside_a_death_hook_is_not_a_forced_removal() {
        let cp = checkpoint(Vec::new());
        let hook_only = evidence(
            vec![
                Sited {
                    root: Root::Trigger {
                        id: "trigger/wake".into(),
                        bearer: Some("npc/host".into()),
                        approach: None,
                    },
                    ..sited(1, 0, 0, &[], Act::Stage("actor/warden".into()))
                },
                sited(7, 2, 3, &[], Act::DespawnNpc("npc/host".into())),
                Sited {
                    optional: true,
                    ..sited(7, 3, 3, &[], Act::RemoveActor("actor/warden".into()))
                },
            ],
            Vec::new(),
        );
        assert!(
            hook_only.bearer_bound(&cp, "actor/warden").is_none(),
            "a despawn the party reaches only by dying closes the death route, never the \
             surviving one"
        );
    }

    /// **Dominance** (spec-0044 §6): the forced path already walks the party into
    /// the force inside this seat's reign, no farther than the seat stands.
    #[test]
    fn a_forced_beat_inside_the_reign_dominates() {
        let ev = evidence(
            vec![
                sited(0, 0, 1, &[], Act::Stage("actor/warden".into())),
                sited(0, 1, 1, &[], Act::Unleash("actor/warden".into())),
            ],
            vec![(4, vec![[0, 64, 2]])],
        );
        let cp = checkpoint(Vec::new()); // fire_step 3
        let credit = ev
            .dominance(&cp, None, "actor/warden", &[[0, 64, 0]], 8.0)
            .expect("a forced step 2 blocks from the body dominates a seat 8 blocks from it");
        assert_eq!(credit.kind, "dominated");
        assert!(credit.reason.contains("step 4") && credit.reason.contains("2.00"));
    }

    /// …and its three red sides, each of which leaves the pair red.
    #[test]
    fn dominance_refuses_a_beat_outside_the_reign_a_farther_one_and_a_fresher_one() {
        let cells = [[0, 64, 0]];
        // (i) the close beat is in a DIFFERENT reign — before this seat is armed,
        //     so it is not a segment a death at this seat re-walks.
        let before = evidence(
            vec![
                sited(0, 0, 1, &[], Act::Stage("actor/warden".into())),
                sited(0, 1, 1, &[], Act::Unleash("actor/warden".into())),
            ],
            vec![(2, vec![[0, 64, 2]])],
        );
        assert!(
            before
                .dominance(&checkpoint(Vec::new()), None, "actor/warden", &cells, 8.0)
                .is_none()
        );
        // …and after the reign has ended.
        let after = evidence(
            vec![
                sited(0, 0, 1, &[], Act::Stage("actor/warden".into())),
                sited(0, 1, 1, &[], Act::Unleash("actor/warden".into())),
            ],
            vec![(9, vec![[0, 64, 2]])],
        );
        assert!(
            after
                .dominance(
                    &checkpoint(Vec::new()),
                    Some(5),
                    "actor/warden",
                    &cells,
                    8.0
                )
                .is_none()
        );
        // (ii) the beat is FARTHER than the seat: the retry is not the gentler
        //      encounter, so nothing is credited.
        let farther = evidence(
            vec![
                sited(0, 0, 1, &[], Act::Stage("actor/warden".into())),
                sited(0, 1, 1, &[], Act::Unleash("actor/warden".into())),
            ],
            vec![(4, vec![[0, 64, 12]])],
        );
        assert!(
            farther
                .dominance(&checkpoint(Vec::new()), None, "actor/warden", &cells, 8.0)
                .is_none()
        );
        // (iii) the beat is FRESHER than the force: it falls before the body can
        //       perceive anything, so it proves nothing about a retry that meets
        //       a live one.
        let fresher = evidence(
            vec![
                sited(0, 0, 1, &[], Act::Stage("actor/warden".into())),
                sited(0, 1, 6, &[], Act::Unleash("actor/warden".into())),
            ],
            vec![(4, vec![[0, 64, 2]])],
        );
        assert!(
            fresher
                .dominance(&checkpoint(Vec::new()), None, "actor/warden", &cells, 8.0)
                .is_none()
        );
        // …and with no stationary cell at all there is nothing to dominate: this
        // is what keeps a lane wave's smeared march corridor red-side only.
        assert!(
            before
                .dominance(&checkpoint(Vec::new()), None, "actor/warden", &[], 8.0)
                .is_none()
        );
    }
}
