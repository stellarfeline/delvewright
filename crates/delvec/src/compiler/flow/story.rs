//! **Story worlds** (spec-0108 §4): a dialogue selection **and a
//! participation** — the set of elective acts performed.
//!
//! Today's worlds ([`Flow::world_count`]) are one per dialogue selection, every
//! elective act performed, and they stay exactly as they are for the codes that
//! read them (`DW0201`–`DW0203`, `DW0879`, the critical-path `DW0204`). The
//! story worlds are a second enumeration, read only by the branch proofs and the
//! ending proofs.
//!
//! ## Acts
//!
//! An **elective act** is an optional quest (spec-0051, performed whole) or an
//! ambient root fired by a player — a trigger, a trap's payload, a trap's or
//! timed gate's disarm, a shortcut's unlock, a shop offer, an `on_kill` —
//! performed once or not at all. A root the mainline itself performs is
//! **forced** ([`Acts::forced`]): performed in every world, its flags credited
//! where its gate holds as today, its datum writes left undatable.
//!
//! ## Worlds
//!
//! Per dialogue selection, in today's order: the *all* world (every elective
//! performed — today's world), the *none* world (none performed), then one
//! **witness** per target a candidate participation realizes, by participation
//! size and act id, deduplicated on (selection, participation) and capped at
//! the bound. A **target** is one value class per input of one final scene
//! (§4.2); a candidate is a subset of the elective acts that write some input,
//! searched in canonical order and bounded. Each candidate is RUN — solved,
//! ordered, and replayed with its entry deeds performed — and what the party
//! holds the moment each final scene begins to fire decides which target it
//! realizes. A datum is computed by the replay, never by an abstract sum, so a
//! `set-state` among the writers is judged as the datapack runs it.

use super::*;

use crate::compiler::ending::{Class, InputKey, Scenes};

/// The most candidate participations one dialogue selection's witness search
/// runs before it stops and says so.
pub const MAX_CANDIDATES: usize = 256;

/// The most targets one scene enumerates per selection before the rest are
/// counted at the bound.
pub const MAX_TARGETS: usize = 4096;

/// One elective act (spec-0108 §4.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Elective {
    /// The act's id (`quest/…`, `trigger/…`, `trap/…`, `trap/…/disarm`,
    /// `timed-gate/…/disarm`, `shortcut/…`, `shop/…/offer-<i>`, `wave/…`,
    /// `actor/…`).
    pub id: String,
    /// What kind of act it is, as a reader names it.
    pub kind: &'static str,
    /// `None` when the harness can perform it; otherwise why not (§6.4).
    pub undrivable: Option<&'static str>,
}

/// How to perform one ambient act at a point of a replay.
pub(super) struct ActBundle<'a> {
    requires: Vec<String>,
    forbids: Vec<String>,
    state: Vec<StateCompare>,
    effects: Option<(String, &'a [QuestEffect])>,
    sets_flag: Option<String>,
}

/// What one ambient root is as an act: its kind, why the harness cannot
/// perform it (if it cannot), its gate's three axes, and whether it is forced.
type ActShape = (
    &'static str,
    Option<&'static str>,
    Vec<String>,
    Vec<String>,
    Vec<StateCompare>,
    bool,
);

/// The acts of a campaign: the elective ones, the forced ones, and how to
/// perform each ambient one.
pub(super) struct Acts<'a> {
    electives: Vec<Elective>,
    elective_ids: BTreeSet<String>,
    forced: BTreeSet<String>,
    bundles: BTreeMap<String, ActBundle<'a>>,
    /// The data a story world dates: every datum this set excludes stays
    /// undatable in a story world.
    story_undatable: BTreeSet<String>,
}

/// Where a story world came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldOrigin {
    /// Every elective act performed — today's world of its selection.
    All,
    /// No elective act performed.
    None,
    /// The first candidate that realized some target.
    Witness,
}

impl WorldOrigin {
    /// The token every artifact spells.
    pub fn token(self) -> &'static str {
        match self {
            WorldOrigin::All => "all",
            WorldOrigin::None => "none",
            WorldOrigin::Witness => "witness",
        }
    }
}

/// One story world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryWorld {
    /// Index of today's world (the dialogue selection) it extends.
    pub selection: usize,
    /// The elective acts it performs, by id.
    pub performs: BTreeSet<String>,
    /// Where it came from.
    pub origin: WorldOrigin,
}

/// One story world, run.
#[derive(Clone, Debug)]
pub struct StoryRun {
    /// Its playthrough, carrying the participation and the entry deeds.
    pub playthrough: Playthrough,
    /// Every effect pointer the run fired, deeds included.
    pub fired: BTreeSet<String>,
    /// The members of `fired` whose gate held only on an undatable term.
    pub undecided: BTreeSet<String>,
    /// What the party held as each final scene began to fire.
    pub scene_at: BTreeMap<String, SceneState>,
    /// `None` when every step was legal where the run took it; otherwise the
    /// first that was not, with why.
    pub incoherent: Option<String>,
    /// The flags the world's fixpoint produces.
    pub flags: BTreeSet<String>,
    /// The quests that complete in it.
    pub completed: BTreeSet<String>,
    /// Elective ambient acts the world performs whose gate never held before
    /// the path ended — named, never silently dropped.
    pub unperformed: Vec<String>,
}

/// One target of one scene under one selection (§4.2).
#[derive(Clone, Debug)]
pub struct Target {
    /// The scene's list pointer.
    pub scene: String,
    /// The dialogue selection.
    pub selection: usize,
    /// One class per input, in the scene's input order.
    pub classes: Vec<(InputKey, Class)>,
    /// The world that realizes it, by index into [`StoryModel::worlds`].
    pub witness: Option<usize>,
    /// Why no world realizes it, when none does.
    pub reason: Option<String>,
}

/// The story worlds of a campaign, with their runs and every target judged.
pub struct StoryModel {
    /// The static reading of the endings these worlds were built for.
    pub scenes: Scenes,
    /// Every elective act, by id.
    pub electives: Vec<Elective>,
    /// Every forced act, by id.
    pub forced: BTreeSet<String>,
    /// The worlds, in canonical order — `world/<k>` is index `k`.
    pub worlds: Vec<StoryWorld>,
    /// Each world's run, aligned with `worlds`.
    pub runs: Vec<StoryRun>,
    /// Every target, per scene, per selection.
    pub targets: Vec<Target>,
    /// The bound the world list is capped at.
    pub bound: usize,
    /// Targets or candidates the search did not examine because a bound was
    /// reached — refused as unproven where an ending needed them.
    pub at_bound: usize,
    /// The data a story world leaves undatable.
    pub undatable: BTreeSet<String>,
    /// The elective acts that write some final scene's input, by id — the acts
    /// whose participation an ending's reading turns on.
    pub relevant: Vec<String>,
}

impl StoryModel {
    /// The worlds whose run fires the effect at `path` — proven, and only on
    /// an undatable term — each by index; only coherent runs count.
    pub fn reaching(&self, path: &str) -> (Vec<usize>, Vec<usize>) {
        let mut proven = Vec::new();
        let mut undecided = Vec::new();
        for (k, r) in self.runs.iter().enumerate() {
            if r.incoherent.is_some() || !r.fired.contains(path) {
                continue;
            }
            if r.undecided.contains(path) {
                undecided.push(k);
            } else {
                proven.push(k);
            }
        }
        (proven, undecided)
    }

    /// Whether act `id` is elective.
    pub fn is_elective(&self, id: &str) -> bool {
        self.electives.iter().any(|e| e.id == id)
    }

    /// The final scene the `campaign-complete` at `path` stands in.
    fn scene_of(&self, path: &str) -> Option<&crate::compiler::ending::Scene> {
        let e = self.scenes.endings.iter().find(|e| e.path == path)?;
        self.scenes.scenes.get(e.scene)
    }

    /// The unrealizable targets of the scene `ending_path` stands in, grouped by
    /// reason, each with its count.
    pub fn reasons_for(&self, ending_path: &str) -> Vec<String> {
        let Some(scene) = self.scene_of(ending_path) else {
            return Vec::new();
        };
        let mut grouped: BTreeMap<String, usize> = BTreeMap::new();
        for t in &self.targets {
            if t.scene != scene.path {
                continue;
            }
            if let Some(r) = &t.reason {
                *grouped.entry(r.clone()).or_insert(0) += 1;
            }
        }
        grouped
            .into_iter()
            .map(|(r, n)| format!("{r} ({n} target(s))"))
            .collect()
    }

    /// The data the scene of `ending_path` reads that no story world dates.
    pub fn undatable_words(&self, ending_path: &str) -> String {
        let Some(scene) = self.scene_of(ending_path) else {
            return "none".to_string();
        };
        let ids: Vec<&str> = scene
            .inputs
            .iter()
            .filter_map(|k| match k {
                InputKey::Datum(d) if self.undatable.contains(d) => Some(d.as_str()),
                _ => None,
            })
            .collect();
        if ids.is_empty() {
            "none".to_string()
        } else {
            ids.join(", ")
        }
    }

    /// Unrealizable targets, counted.
    pub fn unrealizable(&self) -> usize {
        self.targets.iter().filter(|t| t.witness.is_none()).count()
    }
}

impl<'a> Flow<'a> {
    /// The acts, built once.
    pub(super) fn acts(&self) -> &Acts<'a> {
        self.acts.get_or_init(|| self.build_acts())
    }

    /// The story worlds (spec-0108 §4), built once at [`MAX_WORLDS`].
    pub fn story(&self) -> &StoryModel {
        self.story.get_or_init(|| self.build_story(MAX_WORLDS))
    }

    /// The story world the exported critical path walks: the first world that
    /// performs no elective act and whose coherent run takes exactly the
    /// mainline's steps ([`Self::playthrough`]), when there is one (§4.4: the
    /// mainline is the *none* world). Its run dates what the mainline cannot —
    /// an elective act's datum — so what the path can show is read off it.
    pub fn mainline_story_world(&self) -> Option<usize> {
        let main = self.playthrough();
        let story = self.story();
        (0..story.worlds.len()).find(|&k| {
            let run = &story.runs[k];
            story.worlds[k].performs.is_empty()
                && run.incoherent.is_none()
                && run.playthrough.steps == main.steps
        })
    }

    /// [`Self::story`] capped at `bound` worlds instead of [`MAX_WORLDS`] — the
    /// override a test drives the cap with; every refusal it causes names the
    /// bound it was given. Not cached.
    pub fn story_within(&self, bound: usize) -> StoryModel {
        self.build_story(bound)
    }

    /// The elective acts of this campaign.
    pub fn electives(&self) -> &[Elective] {
        &self.acts().electives
    }

    /// Whether a story world performing `performs` credits producer `g`.
    pub(super) fn story_credit(&self, g: &GatedFlag, performs: &BTreeSet<String>) -> bool {
        match &g.act {
            None => default_credit(g),
            Some(a) if self.acts().elective_ids.contains(a) => performs.contains(a),
            // A forced act fires in every world, wherever its gate holds.
            Some(_) => true,
        }
    }

    /// Perform ambient act `id` against `st`: its bundle fired through the one
    /// gate test the replay applies everywhere, or its disarm flag set.
    pub(super) fn perform_act(&self, st: &mut ReplayState, id: &str, pos: usize) {
        let mut complete_at: Option<(usize, String)> = None;
        self.perform_act_ending(st, id, pos, &mut complete_at);
    }

    fn perform_act_ending(
        &self,
        st: &mut ReplayState,
        id: &str,
        pos: usize,
        complete_at: &mut Option<(usize, String)>,
    ) {
        let Some(b) = self.acts().bundles.get(id) else {
            return;
        };
        if let Some(f) = &b.sets_flag {
            st.flags.insert(f.clone());
        }
        if let Some((base, effs)) = &b.effects {
            self.fire(
                effs,
                base,
                st,
                complete_at,
                pos,
                "",
                &Beat::Trigger(id.to_string()),
            );
        }
    }

    /// Whether ambient act `id`'s own gate holds at `st`.
    fn act_enabled(&self, id: &str, st: &ReplayState) -> bool {
        let Some(b) = self.acts().bundles.get(id) else {
            return false;
        };
        b.requires.iter().all(|f| st.flags.contains(f))
            && !b.forbids.iter().any(|f| st.flags.contains(f))
            && b.state.iter().all(|t| {
                match st
                    .state
                    .get(t.state.as_str())
                    .copied()
                    .unwrap_or(Datum::Undatable)
                {
                    Datum::Known(v) => i32::try_from(v).is_ok_and(|v| t.holds(v)),
                    Datum::Undatable => true,
                }
            })
    }

    /// A story world's starting state: its barred quests never active, the
    /// data it dates known, its credited producers saturated.
    fn story_initial(&self, performs: &BTreeSet<String>, barred: &BTreeSet<String>) -> ReplayState {
        let mut st = ReplayState {
            barred: barred.clone(),
            ..ReplayState::default()
        };
        for q in &self.c.quests.content.quests {
            if matches!(q.trigger, Trigger::CampaignStart) && !barred.contains(q.id.as_str()) {
                st.active.insert(q.id.as_str().to_string());
            }
        }
        let undatable = &self.acts().story_undatable;
        for (id, v) in &self.initial {
            let d = if undatable.contains(id) {
                Datum::Undatable
            } else {
                Datum::Known(*v)
            };
            st.state.insert(id.clone(), d);
        }
        self.saturate_ambient_with(&mut st.flags, &|g| self.story_credit(g, performs));
        st
    }

    /// The optional quests a participation does not perform.
    fn barred(&self, performs: &BTreeSet<String>) -> BTreeSet<String> {
        self.c
            .quest_plan
            .content
            .optional()
            .into_iter()
            .filter(|q| !performs.contains(*q))
            .map(str::to_string)
            .collect()
    }

    /// Solve, order and replay one story world.
    pub fn run_story(&self, w: &StoryWorld) -> StoryRun {
        let performs = &w.performs;
        let barred = self.barred(performs);
        let credit = |g: &GatedFlag| self.story_credit(g, performs);
        let sol = self.solve_with(&self.worlds[w.selection], &credit, &barred);
        let flags = sol.flags.clone();
        let completed = sol.completed.clone();
        let mut pt = self.playthrough_from(Some(sol), true);
        let mut st = self.story_initial(performs, &barred);
        // The ambient electives this world performs that write what an ending
        // reads — its entry deeds — in act-id order.
        let relevant = self.relevant_acts();
        let pending: Vec<String> = relevant
            .iter()
            .filter(|a| performs.contains(*a) && self.acts().bundles.contains_key(*a))
            .cloned()
            .collect();
        // Where each deed is performed: in front of the LAST step, before the
        // first final scene fires, at which the act's own gate holds — as late as
        // the reading allows, so a deed beyond a door the mainline opens is
        // performed after the door is open. Found by a dry pass over the same
        // path with no deed performed.
        let mut at: BTreeMap<String, usize> = BTreeMap::new();
        {
            let mut dry = self.story_initial(performs, &barred);
            let mut dry_complete: Option<(usize, String)> = None;
            for (i, step) in pt.steps.iter().enumerate() {
                for act in &pending {
                    if self.act_enabled(act, &dry) {
                        at.insert(act.clone(), i);
                    }
                }
                self.saturate_ambient_with(&mut dry.flags, &credit);
                self.advance_with(&mut dry, step, i + 1, &mut dry_complete, &credit);
                if !dry.scene_at.is_empty() {
                    break;
                }
            }
        }
        let mut unperformed: Vec<String> = pending
            .iter()
            .filter(|a| !at.contains_key(*a))
            .cloned()
            .collect();
        let mut deeds: Vec<(usize, String)> = Vec::new();
        let mut complete_at: Option<(usize, String)> = None;
        let mut fired: BTreeSet<String> = BTreeSet::new();
        let mut undecided: BTreeSet<String> = BTreeSet::new();
        let mut incoherent: Option<String> = None;
        for (i, step) in pt.steps.iter().enumerate() {
            for act in pending.iter().filter(|a| at.get(*a) == Some(&i)) {
                if !self.act_enabled(act, &st) {
                    // An earlier deed shut its gate: it is not performed.
                    unperformed.push(act.clone());
                    continue;
                }
                st.fired.clear();
                st.undecided.clear();
                self.perform_act_ending(&mut st, act, i + 1, &mut complete_at);
                fired.extend(st.fired.iter().cloned());
                undecided.extend(st.undecided.iter().cloned());
                deeds.push((i, act.clone()));
            }
            self.saturate_ambient_with(&mut st.flags, &credit);
            if incoherent.is_none()
                && let Some(why) = self.step_blocked(&st, step)
            {
                incoherent = Some(format!("step {} (`{}`): {why}", i + 1, step.objective));
            }
            self.advance_with(&mut st, step, i + 1, &mut complete_at, &credit);
            fired.extend(st.fired.iter().cloned());
            undecided.extend(st.undecided.iter().cloned());
        }
        pt.story = Some(std::sync::Arc::new(StoryCtx {
            performs: performs.clone(),
            deeds,
        }));
        StoryRun {
            playthrough: pt,
            fired,
            undecided,
            scene_at: st.scene_at,
            incoherent,
            flags,
            completed,
            unperformed,
        }
    }

    /// The elective acts whose writers write some final scene's input, by id.
    fn relevant_acts(&self) -> BTreeSet<String> {
        let scenes = Scenes::of(self.c);
        let acts = self.acts();
        scenes
            .writers
            .values()
            .flatten()
            .filter_map(|w| w.act.clone())
            .filter(|a| acts.elective_ids.contains(a))
            .collect()
    }

    fn build_acts(&self) -> Acts<'a> {
        let c = self.c;
        let content = &c.quests.content;
        let optional = c.quest_plan.content.optional();
        let mut electives: Vec<Elective> = Vec::new();
        let mut bundles: BTreeMap<String, ActBundle<'a>> = BTreeMap::new();
        let mut forced: BTreeSet<String> = BTreeSet::new();
        let names = |fs: &[delvewright_dsl::FlagId]| -> Vec<String> {
            fs.iter().map(|f| f.as_str().to_string()).collect()
        };
        for q in &optional {
            electives.push(Elective {
                id: (*q).to_string(),
                kind: "quest",
                undrivable: None,
            });
        }
        // What the mainline performs: the debts its own path reads, every
        // trigger whose bundle opens a way, every repeatable press a mandatory
        // numeric gate reads, every fight a mandatory `kill` slays.
        let debts: BTreeSet<String> = self
            .trigger_debts(&self.playthrough())
            .into_iter()
            .map(|d| d.trigger)
            .collect();
        let mandatory: BTreeSet<&str> = c
            .quest_plan
            .content
            .quests
            .iter()
            .filter(|q| q.mandatory)
            .map(|q| q.id.as_str())
            .collect();
        let mut gated_data: BTreeSet<&str> = BTreeSet::new();
        let mut slain: BTreeSet<&str> = BTreeSet::new();
        for q in &content.quests {
            if !mandatory.contains(q.id.as_str()) {
                continue;
            }
            for o in &q.objectives {
                for t in o.requires_state() {
                    gated_data.insert(t.state.as_str());
                }
                if let Objective::Kill { wave, .. } = o {
                    slain.insert(wave.as_str());
                }
            }
        }
        delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
            let Some(id) = crate::compiler::ending::act_of_owner(&site.owner, &site.path) else {
                return;
            };
            let (kind, undrivable, requires, forbids, state, is_forced): ActShape =
                match &site.owner {
                    delvewright_dsl::EffectRootOwner::Trigger(t) => {
                        let opens = list.iter().any(|e| {
                            let mut any = false;
                            e.visit_deep(&mut |x| {
                                any |= matches!(x.gate_region_write(), Some((_, false)))
                                    || x.way_write().is_some();
                            });
                            any
                        });
                        let presses = list.iter().any(|e| {
                            e.writes_state()
                                .is_some_and(|(d, _)| gated_data.contains(d.as_str()))
                        });
                        // A repeatable trigger that carries a body is a link the
                        // route proof may take (spec-0083, `link::collect`): the
                        // mainline presses it to cross, so it is never skipped.
                        let link = !t.once
                            && list.iter().any(|e| {
                                let mut any = false;
                                e.visit_deep(&mut |x| any |= x.teleport().is_some());
                                any
                            });
                        (
                            "trigger",
                            None,
                            names(&t.requires_flags),
                            names(&t.forbids_flags),
                            t.requires_state.clone(),
                            debts.contains(t.id.as_str()) || opens || presses || link,
                        )
                    }
                    delvewright_dsl::EffectRootOwner::TrapPayload(t) => (
                        "trap",
                        Some("springing a trap has no harness step kind"),
                        names(&t.requires_flags),
                        names(&t.forbids_flags),
                        Vec::new(),
                        false,
                    ),
                    delvewright_dsl::EffectRootOwner::ShortcutUnlock(_) => (
                        "shortcut",
                        Some("unlocking a shortcut has no harness step kind"),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        false,
                    ),
                    delvewright_dsl::EffectRootOwner::ShopOffer(h) => {
                        let offer = site
                            .path
                            .split('/')
                            .nth(5)
                            .and_then(|n| n.parse::<usize>().ok())
                            .and_then(|n| h.offers.get(n));
                        (
                            "shop-offer",
                            Some("a shop purchase has no harness step kind"),
                            offer.map(|o| names(&o.requires_flags)).unwrap_or_default(),
                            offer.map(|o| names(&o.forbids_flags)).unwrap_or_default(),
                            offer.map(|o| o.requires_state.clone()).unwrap_or_default(),
                            false,
                        )
                    }
                    delvewright_dsl::EffectRootOwner::OnKill(f) => (
                        "on-kill",
                        Some("a credited kill has no harness step kind"),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        slain.contains(f.id()),
                    ),
                    _ => return,
                };
            bundles.insert(
                id.clone(),
                ActBundle {
                    requires,
                    forbids,
                    state,
                    effects: Some((site.path.clone(), list)),
                    sets_flag: None,
                },
            );
            if is_forced {
                forced.insert(id);
            } else if !electives.iter().any(|e| e.id == id) {
                electives.push(Elective {
                    id,
                    kind,
                    undrivable,
                });
            }
        });
        for t in &content.traps {
            if let Some(d) = &t.disarm {
                let id = crate::compiler::ending::disarm_act(t.id.as_str());
                bundles.insert(
                    id.clone(),
                    ActBundle {
                        requires: names(&t.requires_flags),
                        forbids: Vec::new(),
                        state: Vec::new(),
                        effects: None,
                        sets_flag: Some(d.sets_flag.as_str().to_string()),
                    },
                );
                electives.push(Elective {
                    id,
                    kind: "trap-disarm",
                    undrivable: Some("a trap disarm has no harness step kind"),
                });
            }
        }
        for g in &content.timed_gates {
            if let Some(d) = &g.disarm {
                let id = crate::compiler::ending::disarm_act(g.id.as_str());
                bundles.insert(
                    id.clone(),
                    ActBundle {
                        requires: Vec::new(),
                        forbids: Vec::new(),
                        state: Vec::new(),
                        effects: None,
                        sets_flag: Some(d.sets_flag.as_str().to_string()),
                    },
                );
                electives.push(Elective {
                    id,
                    kind: "timed-gate-disarm",
                    undrivable: Some("a timed gate's disarm has no harness step kind"),
                });
            }
        }
        electives.sort_by(|a, b| a.id.cmp(&b.id));
        let elective_ids: BTreeSet<String> = electives.iter().map(|e| e.id.clone()).collect();
        let story_undatable = self.story_undatable(&elective_ids);
        Acts {
            electives,
            elective_ids,
            forced,
            bundles,
            story_undatable,
        }
    }

    /// The data a story world leaves undatable (spec-0108 §4.1): every datum
    /// today's model cannot date, except a datum a final scene reads whose
    /// every undated writer is an elective ambient act — performed once or not
    /// at all, at its entry deed.
    fn story_undatable(&self, electives: &BTreeSet<String>) -> BTreeSet<String> {
        let c = self.c;
        let read: BTreeSet<String> = Scenes::of(c)
            .scenes
            .iter()
            .flat_map(|s| s.inputs.iter())
            .filter_map(|k| match k {
                InputKey::Datum(d) => Some(d.clone()),
                InputKey::Flag(_) => None,
            })
            .collect();
        let mut still: BTreeSet<String> = BTreeSet::new();
        delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
            let dated = match &site.owner {
                delvewright_dsl::EffectRootOwner::ObjectiveComplete { .. }
                | delvewright_dsl::EffectRootOwner::QuestComplete { .. } => true,
                other => crate::compiler::ending::act_of_owner(other, &site.path)
                    .is_some_and(|a| electives.contains(&a)),
            };
            collect_undated_writes(list, !dated, &mut still);
        });
        for s in &c.quests.content.stakes {
            still.insert(s.state.as_str().to_string());
        }
        for l in &c.quests.content.loops {
            if let Some(counts) = &l.counts {
                still.insert(counts.as_str().to_string());
            }
        }
        if crate::compiler::plan::min_players(c) >= 2 {
            for s in &c.quests.content.state {
                if s.scope == StateScope::Player {
                    still.insert(s.id.as_str().to_string());
                }
            }
        }
        self.undatable
            .iter()
            .filter(|d| !read.contains(*d) || still.contains(*d))
            .cloned()
            .collect()
    }

    fn build_story(&self, bound: usize) -> StoryModel {
        let scenes = Scenes::of(self.c);
        let acts = self.acts();
        let all: BTreeSet<String> = acts.elective_ids.clone();
        let relevant: Vec<String> = self.relevant_acts().into_iter().collect();
        let mut worlds: Vec<StoryWorld> = Vec::new();
        let mut runs: Vec<StoryRun> = Vec::new();
        let mut targets: Vec<Target> = Vec::new();
        let mut at_bound = 0usize;
        for sel in 0..self.worlds.len() {
            // Candidate participations: subsets of the relevant electives, by
            // size then act id, bounded.
            let mut candidates: Vec<BTreeSet<String>> = Vec::new();
            let mut truncated = false;
            'sizes: for size in 0..=relevant.len() {
                for combo in combinations(relevant.len(), size) {
                    if candidates.len() >= MAX_CANDIDATES {
                        truncated = true;
                        break 'sizes;
                    }
                    candidates.push(combo.iter().map(|&i| relevant[i].clone()).collect());
                }
            }
            if truncated {
                at_bound += 1;
            }
            let mut local: Vec<(BTreeSet<String>, WorldOrigin, StoryRun)> = Vec::new();
            let run_of = |performs: BTreeSet<String>,
                          origin: WorldOrigin,
                          local: &mut Vec<(BTreeSet<String>, WorldOrigin, StoryRun)>|
             -> usize {
                if let Some(i) = local.iter().position(|(p, _, _)| *p == performs) {
                    return i;
                }
                let run = self.run_story(&StoryWorld {
                    selection: sel,
                    performs: performs.clone(),
                    origin,
                });
                local.push((performs, origin, run));
                local.len() - 1
            };
            let i_all = run_of(all.clone(), WorldOrigin::All, &mut local);
            let i_none = run_of(BTreeSet::new(), WorldOrigin::None, &mut local);
            let mut cand_runs: Vec<usize> = Vec::new();
            for p in candidates {
                cand_runs.push(run_of(p, WorldOrigin::Witness, &mut local));
            }
            // Per scene: what each run realized.
            let mut chosen: BTreeSet<usize> = BTreeSet::new();
            chosen.insert(i_all);
            chosen.insert(i_none);
            let mut scene_targets: Vec<Target> = Vec::new();
            for scene in &scenes.scenes {
                let classes: Vec<Vec<Class>> =
                    scene.inputs.iter().map(|k| scenes.classes(k)).collect();
                // The run each realized target was first seen in, and every
                // (input, class) any run reached.
                let mut realized: BTreeMap<Vec<Class>, usize> = BTreeMap::new();
                let mut seen: BTreeSet<(usize, Class)> = BTreeSet::new();
                let mut values: BTreeMap<usize, BTreeSet<i64>> = BTreeMap::new();
                let mut reached = false;
                let order: Vec<usize> = std::iter::once(i_none)
                    .chain(cand_runs.iter().copied())
                    .chain(std::iter::once(i_all))
                    .collect();
                for &ri in &order {
                    let run = &local[ri].2;
                    if run.incoherent.is_some() {
                        continue;
                    }
                    let Some(snap) = run.scene_at.get(&scene.path) else {
                        continue;
                    };
                    reached = true;
                    let mut target: Vec<Class> = Vec::new();
                    let mut whole = true;
                    for (ii, k) in scene.inputs.iter().enumerate() {
                        let class = match k {
                            InputKey::Flag(f) => Some(if snap.flags.contains(f) {
                                Class::Set
                            } else {
                                Class::Unset
                            }),
                            InputKey::Datum(d) => match snap.data.get(d).copied().flatten() {
                                Some(v) => {
                                    values.entry(ii).or_default().insert(v);
                                    classes[ii]
                                        .iter()
                                        .find(|c| matches!(c, Class::Within(i) if i.contains(v)))
                                        .copied()
                                }
                                None => None,
                            },
                        };
                        match class {
                            Some(cl) => {
                                seen.insert((ii, cl));
                                target.push(cl);
                            }
                            None => whole = false,
                        }
                    }
                    if whole {
                        realized.entry(target).or_insert(ri);
                    }
                }
                let product: usize = classes
                    .iter()
                    .map(Vec::len)
                    .try_fold(1usize, |a, n| a.checked_mul(n))
                    .unwrap_or(usize::MAX);
                if product > MAX_TARGETS {
                    at_bound += 1;
                }
                for combo in product_of(&classes, MAX_TARGETS) {
                    let witness = realized.get(&combo).copied();
                    if let Some(ri) = witness {
                        chosen.insert(ri);
                    }
                    let reason = match witness {
                        Some(_) => None,
                        None if !reached => Some(
                            "no story world's walkable run reaches this scene — its host never \
                             completes, or every run that would is incoherent"
                                .to_string(),
                        ),
                        None => Some(self.unrealizable_reason(
                            &scenes,
                            scene,
                            &combo,
                            &seen,
                            &values,
                            truncated,
                            &acts.story_undatable,
                        )),
                    };
                    scene_targets.push(Target {
                        scene: scene.path.clone(),
                        selection: sel,
                        classes: scene.inputs.iter().cloned().zip(combo).collect(),
                        // Local run index for now; mapped below.
                        witness,
                        reason,
                    });
                }
            }
            // This selection's worlds in canonical order: all, none, then the
            // witnesses by participation size and act id.
            let mut order: Vec<usize> = vec![i_all];
            if i_none != i_all {
                order.push(i_none);
            }
            let mut wit: Vec<usize> = chosen
                .iter()
                .copied()
                .filter(|i| *i != i_all && *i != i_none)
                .collect();
            wit.sort_by(|a, b| {
                let (pa, pb) = (&local[*a].0, &local[*b].0);
                (pa.len(), pa.iter().collect::<Vec<_>>()).cmp(&(pb.len(), pb.iter().collect()))
            });
            order.extend(wit);
            let mut index_of: BTreeMap<usize, usize> = BTreeMap::new();
            let mut locals: Vec<Option<(BTreeSet<String>, WorldOrigin, StoryRun)>> =
                local.into_iter().map(Some).collect();
            for li in order {
                if worlds.len() >= bound {
                    at_bound += 1;
                    continue;
                }
                let (performs, origin, run) = locals[li].take().expect("one world per run");
                index_of.insert(li, worlds.len());
                worlds.push(StoryWorld {
                    selection: sel,
                    performs,
                    origin,
                });
                runs.push(run);
            }
            for mut t in scene_targets {
                t.witness = t.witness.and_then(|li| index_of.get(&li).copied());
                if t.witness.is_none() && t.reason.is_none() {
                    t.reason = Some(format!(
                        "the world that realizes it is past the bound of {bound} world(s)"
                    ));
                }
                targets.push(t);
            }
        }
        StoryModel {
            scenes,
            electives: acts.electives.clone(),
            forced: acts.forced.clone(),
            worlds,
            runs,
            targets,
            bound,
            at_bound,
            undatable: acts.story_undatable.clone(),
            relevant,
        }
    }

    /// Why no run realizes `combo` for `scene` (§4.2).
    #[allow(clippy::too_many_arguments)]
    fn unrealizable_reason(
        &self,
        scenes: &Scenes,
        scene: &crate::compiler::ending::Scene,
        combo: &[Class],
        seen: &BTreeSet<(usize, Class)>,
        values: &BTreeMap<usize, BTreeSet<i64>>,
        truncated: bool,
        undatable: &BTreeSet<String>,
    ) -> String {
        let acts = self.acts();
        let mut why: Vec<String> = Vec::new();
        for (ii, (k, cl)) in scene.inputs.iter().zip(combo).enumerate() {
            if seen.contains(&(ii, *cl)) {
                continue;
            }
            let writers = scenes.writers.get(k).map(Vec::as_slice).unwrap_or(&[]);
            let reason = match (k, cl) {
                (InputKey::Flag(f), Class::Unset) => {
                    match writers.iter().find(|w| {
                        w.act
                            .as_ref()
                            .is_none_or(|a| !acts.elective_ids.contains(a))
                    }) {
                        Some(w) => format!(
                            "`{f}` unset: a mandatory or forced writer sets it (`{}`)",
                            w.path
                        ),
                        None => format!("`{f}` unset: every run that reaches the scene sets it"),
                    }
                }
                (InputKey::Flag(f), Class::Set) => {
                    if writers.is_empty() {
                        format!("`{f}` set: nothing writes it")
                    } else {
                        format!(
                            "`{f}` set: no writer of it fires before the scene in this selection"
                        )
                    }
                }
                (InputKey::Datum(d), Class::Within(iv)) => {
                    if undatable.contains(d) {
                        format!("`{d}` in {}: an undatable writer moves it", iv.words())
                    } else {
                        let got: Vec<String> = values
                            .get(&ii)
                            .map(|s| s.iter().map(i64::to_string).collect())
                            .unwrap_or_default();
                        let part = self.needs_part_of_a_quest(writers, d, iv);
                        match part {
                            Some(q) => format!(
                                "`{d}` in {}: the interval needs part of a quest (`{q}`)",
                                iv.words()
                            ),
                            None => format!(
                                "`{d}` in {}: no subset of elective amounts reaches it \
                                 (reachable: {})",
                                iv.words(),
                                if got.is_empty() {
                                    "none".to_string()
                                } else {
                                    got.join(", ")
                                }
                            ),
                        }
                    }
                }
                _ => format!("`{}` cannot take {}", k.id(), cl.words()),
            };
            why.push(reason);
        }
        let mut out = if why.is_empty() {
            "each input's class is reachable, but no participation leaves them all there at once"
                .to_string()
        } else {
            why.join("; ")
        };
        if truncated {
            out.push_str(&format!(
                "; the search stopped at the bound of {MAX_CANDIDATES} candidate participation(s)"
            ));
        }
        out
    }

    /// The optional quest whose writers, taken one by one rather than whole,
    /// would put an additive datum inside `iv` — `None` when splitting no quest
    /// helps, or the writers are not plain additions.
    fn needs_part_of_a_quest(
        &self,
        writers: &[crate::compiler::ending::Writer],
        datum: &str,
        iv: &crate::compiler::ending::Interval,
    ) -> Option<String> {
        let acts = self.acts();
        let initial = self.initial.get(datum).copied().unwrap_or(0);
        let mut base = initial;
        // (act, amount) per elective writer; any non-additive writer stops it.
        let mut units: Vec<(String, i64)> = Vec::new();
        for w in writers {
            let amount = self.add_amount(&w.path)?;
            match &w.act {
                Some(a) if acts.elective_ids.contains(a) => units.push((a.clone(), amount)),
                _ => base += amount,
            }
        }
        if units.len() > 16 {
            return None;
        }
        let sums = |items: &[i64]| -> BTreeSet<i64> {
            let mut s: BTreeSet<i64> = BTreeSet::from([base]);
            for x in items {
                let next: Vec<i64> = s.iter().map(|v| v + x).collect();
                s.extend(next);
            }
            s
        };
        let mut by_act: BTreeMap<String, i64> = BTreeMap::new();
        for (a, n) in &units {
            *by_act.entry(a.clone()).or_insert(0) += n;
        }
        let whole: Vec<i64> = by_act.values().copied().collect();
        let split: Vec<i64> = units.iter().map(|(_, n)| *n).collect();
        if sums(&whole).iter().any(|v| iv.contains(*v))
            || !sums(&split).iter().any(|v| iv.contains(*v))
        {
            return None;
        }
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for (a, _) in &units {
            *counts.entry(a.as_str()).or_insert(0) += 1;
        }
        counts
            .into_iter()
            .find(|(a, n)| *n > 1 && a.starts_with("quest/"))
            .map(|(a, _)| a.to_string())
    }

    /// The amount of the ungated `add-state` at `path`, if that is what it is.
    fn add_amount(&self, path: &str) -> Option<i64> {
        let mut out = None;
        delvewright_dsl::for_each_campaign_effect(self.c, &mut |p, _, e| {
            if p == path
                && e.when.is_none()
                && let Verb::AddState { amount, .. } = &e.verb
            {
                out = Some(i64::from(*amount));
            }
        });
        out
    }

    /// The flags story world `k` produces.
    pub fn story_flags(&self, k: usize) -> BTreeSet<String> {
        self.story().runs[k].flags.clone()
    }

    /// The quests that complete in story world `k`.
    pub fn story_completed(&self, k: usize) -> BTreeSet<String> {
        self.story().runs[k].completed.clone()
    }

    /// Story world `k`'s playthrough, carrying its participation and deeds.
    pub fn story_playthrough(&self, k: usize) -> Playthrough {
        self.story().runs[k].playthrough.clone()
    }

    /// How many story worlds there are.
    pub fn story_count(&self) -> usize {
        self.story().worlds.len()
    }

    /// The flags each alternative a selection takes sets — the dialogue half of
    /// a story world, in group order.
    pub fn selection_flags(&self, selection: usize) -> Vec<Vec<String>> {
        self.worlds[selection]
            .iter()
            .enumerate()
            .filter_map(|(g, a)| a.map(|a| self.groups[g].alts[a].flags.clone()))
            .collect()
    }

    /// A walk that plays `p` the way its world plays it: a story world's
    /// participation and deeds when `p` carries one, today's stance otherwise.
    /// [`Self::walk_for`] under the **guaranteed** stance
    /// ([`Self::walk_performing`]): an ambient producer is credited only once the
    /// path performs its trigger ([`Walk::perform`]), and a story world's entry
    /// deeds are performed in front of the step its run performs them at. What
    /// the party really holds walking the path — the reading an armed objective
    /// is judged by (`plan::PathFiring::keep_out`).
    pub fn walk_for_performing(&self, p: &Playthrough) -> Walk<'_, 'a> {
        match &p.story {
            None => self.walk_performing(),
            Some(ctx) => {
                let barred = self.barred(&ctx.performs);
                let mut st = self.story_initial(&ctx.performs, &barred);
                st.flags = self.initial_state_with(&|_| false).flags;
                Walk {
                    flow: self,
                    st,
                    complete_at: None,
                    taken: 0,
                    performed: Some(BTreeSet::new()),
                    story: Some(ctx.clone()),
                }
            }
        }
    }

    pub fn walk_for(&self, p: &Playthrough) -> Walk<'_, 'a> {
        match &p.story {
            None => self.walk(),
            Some(ctx) => {
                let barred = self.barred(&ctx.performs);
                Walk {
                    flow: self,
                    st: self.story_initial(&ctx.performs, &barred),
                    complete_at: None,
                    taken: 0,
                    performed: None,
                    story: Some(ctx.clone()),
                }
            }
        }
    }
}

/// Every `size`-subset of `0..n`, in lexicographic order.
fn combinations(n: usize, size: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    fn go(start: usize, n: usize, size: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == size {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            cur.push(i);
            go(i + 1, n, size, cur, out);
            cur.pop();
            if out.len() > MAX_CANDIDATES {
                return;
            }
        }
    }
    go(0, n, size, &mut cur, &mut out);
    out
}

/// The cartesian product of `classes`, in order, at most `cap` entries.
fn product_of(classes: &[Vec<Class>], cap: usize) -> Vec<Vec<Class>> {
    let mut out: Vec<Vec<Class>> = vec![Vec::new()];
    for cs in classes {
        let mut next = Vec::new();
        'outer: for base in &out {
            for c in cs {
                if next.len() >= cap {
                    break 'outer;
                }
                let mut v = base.clone();
                v.push(*c);
                next.push(v);
            }
        }
        out = next;
    }
    out
}
