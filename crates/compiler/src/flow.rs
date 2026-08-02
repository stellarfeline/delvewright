//! Branch-coherent campaign flow model — the sound half of the ADR-0005 static
//! layer, shared by [`crate::analyze`] (reachability diagnostics) and
//! [`crate::plan`] (critical-path export).
//!
//! ## Why this exists
//!
//! The completability proof used to run one monotone fixpoint over a single
//! **union** flag set: every `set-flag` anywhere in the campaign counted as an
//! unconditional producer. Three consequences, all unsound:
//!
//! 1. **Gated producers counted as unconditional.** A `set-flag` carrying
//!    `requires_flags`/`forbids_flags`, or one nested in an `on_caught` /
//!    `on_respawn` reaction bundle, was treated as always-produced — so a
//!    flag-gated objective downstream of it looked reachable when nothing on the
//!    quest DAG can actually produce that flag (false green).
//! 2. **Missing producer classes.** Flags set from an environment trigger, a
//!    dialogue option, or a trap `disarm` were invisible to the model, so
//!    legitimate content died as spurious `DW0203` (false red).
//! 3. **No branch model.** Two dialogue options on one node that set conflicting
//!    flags (`flag/wait` vs `flag/flee`) are mutually exclusive for a real
//!    player, yet the union model let a "playthrough" hold both — and the
//!    exported critical path walked *both* branches in one sequence, firing
//!    `campaign-complete` for one ending in the middle of the other.
//!
//! ## The model
//!
//! A **choice group** is a dialogue node with two or more options that each set
//! at least one flag: taking one option means not taking its siblings, so the
//! options are XOR alternatives. A **world** picks one alternative per group; a
//! campaign has one world per point of the product (capped, see
//! [`MAX_WORLDS`]).
//!
//! Reachability is solved **per world**, by the same monotone fixpoint as
//! before, with a producer model that is conditional on its gating context:
//!
//! | Producer | Available when |
//! |----------|----------------|
//! | `set-flag` in `on_objective_complete[o]` | `o` is completable **and** every gate on the enclosing effect chain is satisfied |
//! | `set-flag` in a quest's `on_complete` | that quest completes, same gate rule |
//! | `set-flag` on a dialogue option | the option is reachable from its tree root through takeable options, and is the world's selected alternative of its group |
//! | `set-flag` in an environment trigger's effects | the trigger's own `requires_flags` are satisfied (a `strike`/`use`/`approach` trigger is player-initiated: ambient, no DAG position) |
//! | a trap's `disarm.sets_flag` | the trap's `requires_flags` are satisfied (ambient, same reasoning) |
//! | `set-flag` in an `on_respawn` / `on_caught` reaction bundle | **never** — reaction bundles fire at statically unknowable times, so they are not producers (the conservative stance [`crate::continuity`] already takes) |
//!
//! A quest/objective is reported unreachable only when it is unreachable in
//! **every** world, so the branch model can only ever make `DW0202`/`DW0203`
//! *more* precise, never looser.
//!
//! `forbids_flags` stays deliberately ignored for *producibility* (the
//! documented v0.6 stance: whether a forbidden flag happens to be set depends on
//! play order, which the existence fixpoint does not model). The compensating
//! stronger check is the **path replay** below, which does have a concrete
//! order and enforces every negative gate at its real position.
//!
//! ## Path replay (`DW0204`)
//!
//! [`Flow::playthrough`] extracts a single consistent playthrough — one world,
//! the quests that complete in it, their objectives in `after` order, and for
//! each `talk-to` the completing dialogue option that belongs to that world.
//! [`Flow::replay`] then walks that sequence step by step through the flag /
//! objective / quest state machine and proves each step is activatable and
//! completable *at its position*, and that `campaign-complete` fires exactly at
//! the final step. A failure is `DW0204` naming the first incoherent step.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{Campaign, DialogueEffect, Objective, QuestEffect, Trigger};

/// Objective-path incoherence (the replay check).
pub const DW_PATH_INCOHERENT: &str = "DW0204";

/// Upper bound on enumerated branch worlds. The product of the *flag-reading*
/// choice groups' arities; groups past the bound stay **unconstrained** (all
/// their alternatives selectable at once — exactly the pre-branch-model
/// behavior), so a pathological campaign degrades to the old precision rather
/// than to a wrong verdict.
pub const MAX_WORLDS: usize = 512;

// ---------------------------------------------------------------------------
// model
// ---------------------------------------------------------------------------

/// A flag produced by some effect, plus the conjunction of flag gates on the
/// effect chain that produces it.
#[derive(Clone, Debug)]
struct GatedFlag {
    flag: String,
    requires: Vec<String>,
}

/// One dialogue option, flattened.
#[derive(Clone, Debug)]
struct OptModel {
    /// Flat 1-based index over `(node, option)` in declaration order — the same
    /// enumeration `plan::OptionPlan::n` uses, so the two agree by construction.
    n: usize,
    node: usize,
    next: Option<String>,
    requires: Vec<String>,
    forbids: Vec<String>,
    sets: Vec<String>,
    completes: Vec<String>,
}

/// One NPC's dialogue tree, flattened.
#[derive(Clone, Debug)]
struct TreeModel {
    npc: String,
    root: String,
    node_ids: Vec<String>,
    index: BTreeMap<String, usize>,
    options: Vec<OptModel>,
}

/// One XOR alternative of a [`ChoiceGroup`]: a flag-setting option.
#[derive(Clone, Debug)]
struct Alt {
    n: usize,
    flags: Vec<String>,
}

/// A dialogue node whose options set conflicting flags — a branch point.
#[derive(Clone, Debug)]
struct ChoiceGroup {
    alts: Vec<Alt>,
}

/// A branch world: per choice group, the selected alternative, or `None` for an
/// unconstrained group (every alternative selectable — see [`MAX_WORLDS`]).
type World = Vec<Option<usize>>;

/// What one world's fixpoint proved.
#[derive(Clone, Debug, Default)]
struct Solution {
    active: BTreeSet<String>,
    completable: BTreeSet<String>,
    completed: BTreeSet<String>,
    flags: BTreeSet<String>,
    /// `talk-to` objective id → the flat dialogue option index that completes it
    /// in this world.
    talk_option: BTreeMap<String, usize>,
}

/// One objective on the exported critical path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathStep {
    /// The stage-5 quest the objective belongs to.
    pub quest: String,
    /// The objective id.
    pub objective: String,
    /// For a `talk-to`, the flat dialogue option index (`plan::OptionPlan::n`)
    /// the path takes — the completing option consistent with the chosen world.
    pub talk_option: Option<usize>,
}

/// A single branch-consistent playthrough: the quests that complete in the
/// chosen world, and their objectives in a legal order.
#[derive(Clone, Debug)]
pub struct Playthrough {
    /// Completing quests in `depends_on` topological order.
    pub quests: Vec<String>,
    /// Every objective of those quests, in path order.
    pub steps: Vec<PathStep>,
    /// `true` if the stage-4 `depends_on` graph did not fully linearize (a cycle
    /// `DW0130` should have rejected) — the caller raises its own internal
    /// invariant error.
    pub cyclic: bool,
    /// `true` when no branch completes the finale (already `DW0201`) and this is
    /// the degenerate whole-closure fallback rather than a proven playthrough.
    pub degenerate: bool,
}

/// The evolving state of one playthrough replay: which flags are set, which
/// objectives/quests are done, which quests are active.
#[derive(Clone, Debug, Default)]
struct ReplayState {
    flags: BTreeSet<String>,
    done_obj: BTreeSet<String>,
    done_quest: BTreeSet<String>,
    active: BTreeSet<String>,
}

/// A proven n-agent division of labour (spec-0018).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Division {
    /// The AND-join objective whose arms the party splits.
    pub join: String,
    /// The arms assigned to each agent (`agents.len() == min_players`), by the
    /// deterministic round-robin over the join's free arms.
    pub agents: Vec<Vec<String>>,
}

/// Why a declared party size has no division of labour to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DivisionFailure {
    /// The declared `min_players`.
    pub party: usize,
    /// The most independently-reachable arms any single AND-join offers.
    pub widest: usize,
    /// The join that offered them (`None` when the campaign has no AND-join).
    pub join: Option<String>,
}

impl DivisionFailure {
    /// The `DW0358` diagnostic message.
    pub fn message(&self) -> String {
        let n = self.party;
        match &self.join {
            Some(j) => format!(
                "`min_players: {n}` declares a delve that REQUIRES {n} players, but no AND-join \
                 in the campaign gives {n} of them independent work: the widest join (`{j}`) \
                 offers only {} arm(s) that are reachable at the same moment — the rest wait on a \
                 sibling arm, a flag an earlier arm sets, or a quest that is not active yet. \
                 Split one beat into {n} `after`-arms that are each completable from the join's \
                 frontier, or lower `min_players`",
                self.widest
            ),
            None => format!(
                "`min_players: {n}` declares a delve that REQUIRES {n} players, but the campaign \
                 has no AND-join at all — every objective has at most one `after` prerequisite, \
                 so the whole delve is one serial chain that a single player walks. Give the \
                 party parallel work (an objective with {n} `after` arms in {n} places), or lower \
                 `min_players`"
            ),
        }
    }
}

/// Why a playthrough's step sequence is not a legal playthrough.
#[derive(Clone, Debug)]
pub struct ReplayFailure {
    /// 1-based position of the first incoherent objective on the path.
    pub position: usize,
    /// The objective at that position (or the finale objective, for a
    /// `campaign-complete` placement failure).
    pub objective: String,
    /// Human-readable reason, already phrased as a remedy.
    pub reason: String,
}

impl ReplayFailure {
    /// The `DW0204` diagnostic message.
    pub fn message(&self) -> String {
        format!(
            "the exported critical path is not a playthrough any player can walk: at path step \
             #{} (objective `{}`), {}. Every step of the critical path must be activatable and \
             completable at its position, and `campaign-complete` must fire exactly at the final \
             step — split mutually exclusive endings behind flags so exactly one of them lies on \
             the path, and make each gating flag producible before the step that reads it",
            self.position, self.objective, self.reason
        )
    }
}

// ---------------------------------------------------------------------------
// construction
// ---------------------------------------------------------------------------

/// The branch/flag flow model of a campaign.
pub struct Flow<'a> {
    c: &'a Campaign,
    groups: Vec<ChoiceGroup>,
    /// `(npc, flat option index)` → choice group index.
    opt_group: BTreeMap<(String, usize), usize>,
    trees: Vec<TreeModel>,
    /// Producers with no DAG position: environment triggers and trap disarms.
    ambient: Vec<GatedFlag>,
    obj_flags: BTreeMap<String, Vec<GatedFlag>>,
    quest_flags: BTreeMap<String, Vec<GatedFlag>>,
    worlds: Vec<World>,
}

impl<'a> Flow<'a> {
    /// Build the model. Pure and deterministic (BTree ordering throughout).
    pub fn new(c: &'a Campaign) -> Self {
        let trees = flatten_trees(c);
        let (groups, opt_group) = choice_groups(&trees);
        let mut ambient = Vec::new();
        for t in &c.quests.content.triggers {
            let gate: Vec<String> = t.requires_flags.iter().map(|f| f.as_str().into()).collect();
            collect_flags(&t.effects, &gate, &mut ambient);
        }
        for trap in &c.quests.content.traps {
            if let Some(d) = &trap.disarm {
                ambient.push(GatedFlag {
                    flag: d.sets_flag.as_str().to_string(),
                    requires: trap
                        .requires_flags
                        .iter()
                        .map(|f| f.as_str().into())
                        .collect(),
                });
            }
        }

        let mut obj_flags: BTreeMap<String, Vec<GatedFlag>> = BTreeMap::new();
        let mut quest_flags: BTreeMap<String, Vec<GatedFlag>> = BTreeMap::new();
        for q in &c.quests.content.quests {
            for (oid, effs) in &q.on_objective_complete {
                let mut out = Vec::new();
                collect_flags(effs, &[], &mut out);
                if !out.is_empty() {
                    obj_flags
                        .entry(oid.as_str().to_string())
                        .or_default()
                        .extend(out);
                }
            }
            let mut out = Vec::new();
            collect_flags(&q.on_complete, &[], &mut out);
            if !out.is_empty() {
                quest_flags.insert(q.id.as_str().to_string(), out);
            }
        }

        let worlds = enumerate_worlds(&groups, &read_flags(c));
        Flow {
            c,
            groups,
            opt_group,
            trees,
            ambient,
            obj_flags,
            quest_flags,
            worlds,
        }
    }

    /// Quest ids active in at least one world.
    pub fn any_active(&self) -> BTreeSet<String> {
        self.union(|s| &s.active)
    }

    /// Objective ids completable in at least one world.
    pub fn any_completable(&self) -> BTreeSet<String> {
        self.union(|s| &s.completable)
    }

    /// Quest ids completing in at least one world.
    pub fn any_completed(&self) -> BTreeSet<String> {
        self.union(|s| &s.completed)
    }

    fn union(&self, pick: impl Fn(&Solution) -> &BTreeSet<String>) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for w in &self.worlds {
            out.extend(pick(&self.solve(w)).iter().cloned());
        }
        out
    }

    /// The exported playthrough: the **first** world (in the deterministic
    /// enumeration order) whose finale quest completes, restricted to the quests
    /// that complete in it.
    ///
    /// When no world completes the finale the campaign is already `DW0201` and
    /// there is no coherent playthrough to pick; the model then **degenerates to
    /// the pre-branch behavior** — the finale's whole stage-4 `depends_on`
    /// closure, each `talk-to` left to the first completing option — so the
    /// geometry-only commands (`chart`, `snapshot`) that deliberately run on an
    /// unanalyzable campaign keep working. [`Playthrough::degenerate`] marks it.
    pub fn playthrough(&self) -> Playthrough {
        let finale = self.c.quest_plan.content.finale.as_str();
        let sol = self
            .worlds
            .iter()
            .map(|w| self.solve(w))
            .find(|s| s.completed.contains(finale));
        let degenerate = sol.is_none();
        let keep: BTreeSet<String> = match &sol {
            Some(s) => s.completed.clone(),
            None => self
                .c
                .quest_plan
                .content
                .quests
                .iter()
                .map(|q| q.id.as_str().to_string())
                .collect(),
        };
        let (order, cyclic) = finale_order(self.c, &keep);
        let mut steps = Vec::new();
        for qid in &order {
            let Some(q) = self.quest(qid) else { continue };
            for obj in objectives_in_order(&q.objectives) {
                let oid = obj.id().as_str().to_string();
                let talk_option = match (&sol, obj) {
                    (Some(s), Objective::TalkTo { .. }) => s.talk_option.get(&oid).copied(),
                    _ => None,
                };
                steps.push(PathStep {
                    quest: qid.clone(),
                    objective: oid,
                    talk_option,
                });
            }
        }
        Playthrough {
            quests: order,
            steps,
            cyclic,
            degenerate,
        }
    }

    /// Replay `p`'s step sequence through the flag/objective/quest state machine.
    /// `Ok(())` means every step is activatable and completable at its position
    /// and `campaign-complete` fires exactly at the final step.
    pub fn replay(&self, p: &Playthrough) -> Result<(), ReplayFailure> {
        let mut st8 = self.initial_state();
        let mut complete_at: Option<(usize, String)> = None;

        for (i, st) in p.steps.iter().enumerate() {
            let pos = i + 1;
            let fail = |reason: String| {
                Err(ReplayFailure {
                    position: pos,
                    objective: st.objective.clone(),
                    reason,
                })
            };
            let Some(quest) = self.quest(&st.quest) else {
                return fail(format!("its quest `{}` does not exist", st.quest));
            };
            let Some(obj) = quest
                .objectives
                .iter()
                .find(|o| o.id().as_str() == st.objective)
            else {
                return fail("it is not an objective of its quest".to_string());
            };
            if !st8.active.contains(&st.quest) {
                return fail(format!(
                    "its quest `{}` is not active yet — nothing earlier on the path completes the \
                     quest its trigger names",
                    st.quest
                ));
            }
            for a in obj.after() {
                if !st8.done_obj.contains(a.as_str()) {
                    return fail(format!(
                        "its `after` prerequisite `{}` has not been completed at this point",
                        a.as_str()
                    ));
                }
            }
            for f in obj.requires_flags() {
                if !st8.flags.contains(f.as_str()) {
                    return fail(format!(
                        "it requires `{}`, which nothing earlier on the path sets (a mutually \
                         exclusive branch produces it)",
                        f.as_str()
                    ));
                }
            }
            for f in obj.forbids_flags() {
                if st8.flags.contains(f.as_str()) {
                    return fail(format!(
                        "it forbids `{}`, which an earlier step on the path has already set",
                        f.as_str()
                    ));
                }
            }
            if let Objective::TalkTo { npc, .. } = obj {
                let Some(n) = st.talk_option else {
                    return fail(format!(
                        "no dialogue option of `{}` completes it in the chosen branch",
                        npc.as_str()
                    ));
                };
                if !self.option_takeable_now(npc.as_str(), n, &st8.flags) {
                    return fail(format!(
                        "the completing dialogue option of `{}` is not reachable at this point — \
                         a node or option on the way to it is still flag-gated",
                        npc.as_str()
                    ));
                }
            }

            self.advance(&mut st8, st, pos, &mut complete_at);
        }

        let last = p.steps.len();
        match complete_at {
            Some((at, ref obj)) if at == last => Ok(()),
            Some((at, obj)) => Err(ReplayFailure {
                position: at,
                objective: obj,
                reason: format!(
                    "`campaign-complete` fires here, {} step(s) before the end of the path — the \
                     delve would end mid-playthrough. This is the signature of two mutually \
                     exclusive endings sharing one path",
                    last - at
                ),
            }),
            None => Err(ReplayFailure {
                position: last,
                objective: p
                    .steps
                    .last()
                    .map(|s| s.objective.clone())
                    .unwrap_or_default(),
                reason: "`campaign-complete` never fires on the path — no completion bundle on it \
                         ends the delve"
                    .to_string(),
            }),
        }
    }

    /// A concrete n-agent **division of labour** for the declared party size
    /// (spec-0018), or why none exists.
    ///
    /// Party progression makes an `after: [obj/a, obj/b]` AND-join the primitive
    /// two players split: A clears one arm, B the other, and the successor's guard
    /// — every term a `#party` read — opens for both. A campaign that *declares*
    /// `min_players: n` is claiming its beats need n players, and this is the
    /// machine-checkable content of that claim: somewhere on the proven
    /// playthrough there must be a join with **n arms that are independently
    /// reachable at the join's frontier** — the replay state just before the
    /// earliest arm — with no arm waiting on a sibling. Assignment is then the
    /// deterministic round-robin over that arm list.
    ///
    /// `min_players: 1` never calls this: a party of one is the single-agent
    /// proof [`Self::replay`] already gives, and every pre-spec-0018 campaign
    /// keeps exactly that verdict.
    pub fn divide(&self, p: &Playthrough, n: usize) -> Result<Division, DivisionFailure> {
        let mut st8 = self.initial_state();
        let mut complete_at: Option<(usize, String)> = None;
        // The step position at which each objective completes on the path, and
        // the frontier state captured before it.
        let mut best: Option<Division> = None;
        let mut widest = 0usize;
        let mut widest_join: Option<String> = None;

        // Where each objective sits on the path (1-based), so a join's frontier
        // is the state before its EARLIEST arm.
        let pos_of: BTreeMap<&str, usize> = p
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| (s.objective.as_str(), i + 1))
            .collect();

        for (i, st) in p.steps.iter().enumerate() {
            let pos = i + 1;
            // Every join whose earliest arm completes at THIS position: the
            // current state is that join's frontier (nothing of the join is done).
            for (join, arms) in self.and_joins() {
                let earliest = arms.iter().filter_map(|a| pos_of.get(a.as_str())).min();
                if earliest != Some(&pos) {
                    continue;
                }
                let free: Vec<String> = arms
                    .iter()
                    .filter(|a| self.arm_is_free(a, &st8, &arms))
                    .cloned()
                    .collect();
                if free.len() > widest {
                    widest = free.len();
                    widest_join = Some(join.clone());
                }
                if free.len() >= n && best.is_none() {
                    let mut agents: Vec<Vec<String>> = vec![Vec::new(); n];
                    for (k, arm) in free.iter().enumerate() {
                        agents[k % n].push(arm.clone());
                    }
                    best = Some(Division {
                        join: join.clone(),
                        agents,
                    });
                }
            }
            self.advance(&mut st8, st, pos, &mut complete_at);
        }

        best.ok_or(DivisionFailure {
            party: n,
            widest,
            join: widest_join,
        })
    }

    /// Every AND-join on the campaign: `(objective id, its `after` arms)` for each
    /// objective with two or more prerequisites, in deterministic content order.
    fn and_joins(&self) -> Vec<(String, Vec<String>)> {
        let mut out = Vec::new();
        for q in &self.c.quests.content.quests {
            for o in &q.objectives {
                if o.after().len() >= 2 {
                    out.push((
                        o.id().as_str().to_string(),
                        o.after().iter().map(|a| a.as_str().to_string()).collect(),
                    ));
                }
            }
        }
        out
    }

    /// Can one agent take `arm` starting from the join's frontier state, without
    /// waiting for any SIBLING arm? (Quest active, own `after` already done,
    /// flag gates satisfied, and for a `talk-to` a takeable completing option.)
    fn arm_is_free(&self, arm: &str, st: &ReplayState, siblings: &[String]) -> bool {
        let Some((quest, obj)) = self.c.quests.content.quests.iter().find_map(|q| {
            q.objectives
                .iter()
                .find(|o| o.id().as_str() == arm)
                .map(|o| (q, o))
        }) else {
            return false;
        };
        if !st.active.contains(quest.id.as_str()) {
            return false;
        }
        if obj
            .after()
            .iter()
            .any(|a| siblings.iter().any(|s| s == a.as_str()) || !st.done_obj.contains(a.as_str()))
        {
            return false;
        }
        if obj
            .requires_flags()
            .iter()
            .any(|f| !st.flags.contains(f.as_str()))
        {
            return false;
        }
        if obj
            .forbids_flags()
            .iter()
            .any(|f| st.flags.contains(f.as_str()))
        {
            return false;
        }
        if let Objective::TalkTo { npc, .. } = obj {
            let takeable = self
                .worlds
                .iter()
                .map(|w| self.solve(w))
                .filter_map(|s| s.talk_option.get(arm).copied())
                .any(|k| self.option_takeable_now(npc.as_str(), k, &st.flags));
            if !takeable {
                return false;
            }
        }
        true
    }

    // -- internals ---------------------------------------------------------

    /// The replay's starting state: campaign-start quests active, ambient
    /// (trigger / trap-disarm) flags saturated.
    fn initial_state(&self) -> ReplayState {
        let mut st = ReplayState::default();
        for q in &self.c.quests.content.quests {
            if matches!(q.trigger, Trigger::CampaignStart) {
                st.active.insert(q.id.as_str().to_string());
            }
        }
        self.saturate_ambient(&mut st.flags);
        st
    }

    /// Apply one path step: complete its objective, fire its effect bundles,
    /// cascade quest completions and re-saturate ambient producers. The single
    /// transition function of the replay state machine — shared by [`Self::replay`]
    /// (which checks each step's guards first) and [`Self::divide`] (which snapshots
    /// the state before each step).
    fn advance(
        &self,
        st: &mut ReplayState,
        step: &PathStep,
        pos: usize,
        complete_at: &mut Option<(usize, String)>,
    ) {
        st.done_obj.insert(step.objective.clone());
        if let Some(n) = step.talk_option {
            for f in self.option_sets(&step.objective, n) {
                st.flags.insert(f);
            }
        }
        if let Some(quest) = self.quest(&step.quest)
            && let Some(effs) = quest
                .on_objective_complete
                .get(&delvewright_dsl::ObjectiveId(step.objective.clone()))
        {
            self.fire(effs, &mut st.flags, complete_at, pos, &step.objective);
        }
        // Quest completion cascade.
        loop {
            let mut progressed = false;
            for q in &self.c.quests.content.quests {
                let qid = q.id.as_str();
                if st.done_quest.contains(qid) || !st.active.contains(qid) {
                    continue;
                }
                if !q
                    .objectives
                    .iter()
                    .all(|o| st.done_obj.contains(o.id().as_str()))
                {
                    continue;
                }
                st.done_quest.insert(qid.to_string());
                self.fire(
                    &q.on_complete,
                    &mut st.flags,
                    complete_at,
                    pos,
                    &step.objective,
                );
                for other in &self.c.quests.content.quests {
                    if let Trigger::QuestComplete { quest } = &other.trigger
                        && quest.as_str() == qid
                    {
                        st.active.insert(other.id.as_str().to_string());
                    }
                }
                progressed = true;
            }
            if !progressed {
                break;
            }
        }
        self.saturate_ambient(&mut st.flags);
    }

    fn quest(&self, id: &str) -> Option<&'a delvewright_dsl::Quest> {
        self.c
            .quests
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == id)
    }

    /// Flags a dialogue option sets, looked up by the objective it completes.
    fn option_sets(&self, objective: &str, n: usize) -> Vec<String> {
        for t in &self.trees {
            if let Some(o) = t.options.iter().find(|o| o.n == n)
                && o.completes.iter().any(|c| c == objective)
            {
                return o.sets.clone();
            }
        }
        Vec::new()
    }

    /// Is option `n` of `npc` reachable from the tree root through options whose
    /// own gates `flags` satisfies? (Branch selection is not applied here: the
    /// replay already committed to one option per `talk-to`.)
    fn option_takeable_now(&self, npc: &str, n: usize, flags: &BTreeSet<String>) -> bool {
        let Some(t) = self.trees.iter().find(|t| t.npc == npc) else {
            return false;
        };
        let reach = reachable_nodes(t, &|o: &OptModel| {
            o.requires.iter().all(|f| flags.contains(f))
                && !o.forbids.iter().any(|f| flags.contains(f))
        });
        t.options.iter().any(|o| {
            o.n == n
                && reach.contains(&o.node)
                && o.requires.iter().all(|f| flags.contains(f))
                && !o.forbids.iter().any(|f| flags.contains(f))
        })
    }

    /// Add every ambient (trigger / trap-disarm) flag whose gate is satisfied,
    /// to fixpoint.
    fn saturate_ambient(&self, flags: &mut BTreeSet<String>) {
        loop {
            let mut changed = false;
            for g in &self.ambient {
                if g.requires.iter().all(|f| flags.contains(f)) && flags.insert(g.flag.clone()) {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Fire an effect bundle at a concrete point in the replay: honor every flag
    /// gate against the current state, descend into `sequence` steps and
    /// `on_arrive` bundles, and skip `on_respawn`/`on_caught` reaction bundles.
    fn fire(
        &self,
        effs: &[QuestEffect],
        flags: &mut BTreeSet<String>,
        complete_at: &mut Option<(usize, String)>,
        pos: usize,
        objective: &str,
    ) {
        for e in effs {
            let gated = !e
                .requires_flags()
                .iter()
                .all(|f| flags.contains(f.as_str()))
                || e.forbids_flags().iter().any(|f| flags.contains(f.as_str()));
            if gated {
                continue;
            }
            match e {
                QuestEffect::SetFlag { flag, .. } => {
                    flags.insert(flag.as_str().to_string());
                }
                QuestEffect::CampaignComplete => {
                    if complete_at.is_none() {
                        *complete_at = Some((pos, objective.to_string()));
                    }
                }
                QuestEffect::SetCheckpoint { .. }
                | QuestEffect::Bonfire { .. }
                | QuestEffect::BeginStealth { .. } => continue,
                _ => {}
            }
            for list in e.nested_effect_lists() {
                self.fire(list, flags, complete_at, pos, objective);
            }
        }
    }

    /// The monotone fixpoint, restricted to one branch world.
    fn solve(&self, world: &World) -> Solution {
        let mut s = Solution::default();
        loop {
            let mut changed = false;
            self.saturate_ambient_tracked(&mut s.flags, &mut changed);

            // Dialogue: options reachable from the root through takeable options,
            // restricted to this world's selected alternatives.
            let mut dlg_completes: BTreeMap<String, usize> = BTreeMap::new();
            let snapshot = s.flags.clone();
            for t in &self.trees {
                let takeable = |o: &OptModel| {
                    o.requires.iter().all(|f| snapshot.contains(f))
                        && self.selectable(&t.npc, o.n, world)
                };
                let reach = reachable_nodes(t, &takeable);
                for o in &t.options {
                    if !reach.contains(&o.node) || !takeable(o) {
                        continue;
                    }
                    for f in &o.sets {
                        if s.flags.insert(f.clone()) {
                            changed = true;
                        }
                    }
                    for c in &o.completes {
                        dlg_completes.entry(c.clone()).or_insert(o.n);
                    }
                }
            }

            for q in &self.c.quests.content.quests {
                let qid = q.id.as_str();
                if s.active.contains(qid) {
                    continue;
                }
                let now = match &q.trigger {
                    Trigger::CampaignStart => true,
                    Trigger::QuestComplete { quest } => s.completed.contains(quest.as_str()),
                };
                if now {
                    s.active.insert(qid.to_string());
                    changed = true;
                }
            }

            for q in &self.c.quests.content.quests {
                if !s.active.contains(q.id.as_str()) {
                    continue;
                }
                for obj in &q.objectives {
                    let oid = obj.id().as_str();
                    if s.completable.contains(oid) {
                        continue;
                    }
                    if !obj
                        .after()
                        .iter()
                        .all(|a| s.completable.contains(a.as_str()))
                    {
                        continue;
                    }
                    if !obj
                        .requires_flags()
                        .iter()
                        .all(|f| s.flags.contains(f.as_str()))
                    {
                        continue;
                    }
                    if let Objective::TalkTo { .. } = obj {
                        let Some(&n) = dlg_completes.get(oid) else {
                            continue;
                        };
                        s.talk_option.insert(oid.to_string(), n);
                    }
                    s.completable.insert(oid.to_string());
                    produce(self.obj_flags.get(oid), &mut s.flags, &mut changed);
                    changed = true;
                }
            }

            for q in &self.c.quests.content.quests {
                let qid = q.id.as_str();
                if s.completed.contains(qid) || !s.active.contains(qid) {
                    continue;
                }
                if !q
                    .objectives
                    .iter()
                    .all(|o| s.completable.contains(o.id().as_str()))
                {
                    continue;
                }
                s.completed.insert(qid.to_string());
                produce(self.quest_flags.get(qid), &mut s.flags, &mut changed);
                changed = true;
            }

            if !changed {
                break;
            }
        }
        s
    }

    fn saturate_ambient_tracked(&self, flags: &mut BTreeSet<String>, changed: &mut bool) {
        for g in &self.ambient {
            if g.requires.iter().all(|f| flags.contains(f)) && flags.insert(g.flag.clone()) {
                *changed = true;
            }
        }
    }

    /// Is dialogue option `n` of `npc` the alternative this world selected? An
    /// option that sets no flag belongs to no group and is always selectable.
    fn selectable(&self, npc: &str, n: usize, world: &World) -> bool {
        match self.opt_group.get(&(npc.to_string(), n)) {
            None => true,
            Some(&g) => match world[g] {
                None => true,
                Some(a) => self.groups[g].alts[a].n == n,
            },
        }
    }
}

fn produce(src: Option<&Vec<GatedFlag>>, flags: &mut BTreeSet<String>, changed: &mut bool) {
    let Some(list) = src else { return };
    for g in list {
        if g.requires.iter().all(|f| flags.contains(f)) && flags.insert(g.flag.clone()) {
            *changed = true;
        }
    }
}

/// Every `set-flag` in `effs`, carrying the conjunction of `gate` and the flag
/// gates of the enclosing effect chain. Reaction bundles (`on_respawn` /
/// `on_caught`) are NOT descended: they fire at statically unknowable times, so
/// nothing inside them is a producer.
fn collect_flags(effs: &[QuestEffect], gate: &[String], out: &mut Vec<GatedFlag>) {
    for e in effs {
        let mut here: Vec<String> = gate.to_vec();
        here.extend(e.requires_flags().iter().map(|f| f.as_str().to_string()));
        here.sort();
        here.dedup();
        if let QuestEffect::SetFlag { flag, .. } = e {
            out.push(GatedFlag {
                flag: flag.as_str().to_string(),
                requires: here.clone(),
            });
        }
        match e {
            QuestEffect::SetCheckpoint { .. }
            | QuestEffect::Bonfire { .. }
            | QuestEffect::BeginStealth { .. } => continue,
            _ => {
                for list in e.nested_effect_lists() {
                    collect_flags(list, &here, out);
                }
            }
        }
    }
}

/// Flatten every stage-6 dialogue tree, assigning each option the same flat
/// 1-based index `plan::plan_npc` assigns.
fn flatten_trees(c: &Campaign) -> Vec<TreeModel> {
    let mut out = Vec::new();
    for tree in &c.dialogue.content.dialogues {
        let mut options = Vec::new();
        let mut node_ids = Vec::new();
        let mut index = BTreeMap::new();
        let mut n = 0usize;
        for (ni, node) in tree.nodes.iter().enumerate() {
            node_ids.push(node.id.as_str().to_string());
            index.insert(node.id.as_str().to_string(), ni);
            for opt in &node.options {
                n += 1;
                let mut sets = Vec::new();
                let mut completes = Vec::new();
                for e in &opt.effects {
                    match e {
                        DialogueEffect::SetFlag { flag } => sets.push(flag.as_str().to_string()),
                        DialogueEffect::CompleteObjective { objective } => {
                            completes.push(objective.as_str().to_string());
                        }
                        _ => {}
                    }
                }
                options.push(OptModel {
                    n,
                    node: ni,
                    next: opt.next.as_ref().map(|d| d.as_str().to_string()),
                    requires: opt
                        .requires_flags
                        .iter()
                        .map(|f| f.as_str().into())
                        .collect(),
                    forbids: opt
                        .forbids_flags
                        .iter()
                        .map(|f| f.as_str().into())
                        .collect(),
                    sets,
                    completes,
                });
            }
        }
        out.push(TreeModel {
            npc: tree.npc.as_str().to_string(),
            root: tree.root.as_str().to_string(),
            node_ids,
            index,
            options,
        });
    }
    out
}

/// Node indices reachable from the tree root, traversing only options `takeable`
/// admits.
fn reachable_nodes(t: &TreeModel, takeable: &dyn Fn(&OptModel) -> bool) -> BTreeSet<usize> {
    let mut seen = BTreeSet::new();
    let Some(&root) = t.index.get(&t.root) else {
        return seen;
    };
    let mut queue: VecDeque<usize> = VecDeque::new();
    queue.push_back(root);
    seen.insert(root);
    while let Some(ni) = queue.pop_front() {
        for o in t.options.iter().filter(|o| o.node == ni) {
            if !takeable(o) {
                continue;
            }
            if let Some(next) = &o.next
                && let Some(&nx) = t.index.get(next)
                && seen.insert(nx)
            {
                queue.push_back(nx);
            }
        }
    }
    seen
}

/// The XOR choice groups: dialogue nodes with ≥2 flag-setting options.
fn choice_groups(trees: &[TreeModel]) -> (Vec<ChoiceGroup>, BTreeMap<(String, usize), usize>) {
    let mut groups = Vec::new();
    let mut map = BTreeMap::new();
    for t in trees {
        for (ni, node_id) in t.node_ids.iter().enumerate() {
            let alts: Vec<Alt> = t
                .options
                .iter()
                .filter(|o| o.node == ni && !o.sets.is_empty())
                .map(|o| Alt {
                    n: o.n,
                    flags: o.sets.clone(),
                })
                .collect();
            if alts.len() < 2 {
                continue;
            }
            let _ = node_id;
            let g = groups.len();
            for a in &alts {
                map.insert((t.npc.clone(), a.n), g);
            }
            groups.push(ChoiceGroup { alts });
        }
    }
    (groups, map)
}

/// Every flag read by a gate anywhere in the campaign — objectives, effects
/// (deep, including reaction bundles), dialogue options, triggers, traps. A
/// choice group none of whose flags is read cannot change any reachability
/// verdict, so it never participates in world enumeration.
fn read_flags(c: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let eat = |fs: &[delvewright_dsl::FlagId], out: &mut BTreeSet<String>| {
        for f in fs {
            out.insert(f.as_str().to_string());
        }
    };
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            eat(o.requires_flags(), &mut out);
            eat(o.forbids_flags(), &mut out);
        }
        let deep = |effs: &[QuestEffect], out: &mut BTreeSet<String>| {
            for e in effs {
                e.visit_deep(&mut |x| {
                    for f in x.requires_flags().iter().chain(x.forbids_flags()) {
                        out.insert(f.as_str().to_string());
                    }
                });
            }
        };
        for effs in q.on_objective_complete.values() {
            deep(effs, &mut out);
        }
        deep(&q.on_complete, &mut out);
    }
    for t in &c.quests.content.triggers {
        eat(&t.requires_flags, &mut out);
        eat(&t.forbids_flags, &mut out);
        for e in &t.effects {
            e.visit_deep(&mut |x| {
                for f in x.requires_flags().iter().chain(x.forbids_flags()) {
                    out.insert(f.as_str().to_string());
                }
            });
        }
    }
    for t in &c.quests.content.traps {
        eat(&t.requires_flags, &mut out);
        eat(&t.forbids_flags, &mut out);
    }
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for o in &node.options {
                eat(&o.requires_flags, &mut out);
                eat(&o.forbids_flags, &mut out);
            }
        }
    }
    out
}

/// The branch worlds to solve, in deterministic order (all-first-alternative
/// first). Only groups whose flags are actually read participate; the product is
/// capped at [`MAX_WORLDS`], and groups past the cap stay unconstrained.
fn enumerate_worlds(groups: &[ChoiceGroup], read: &BTreeSet<String>) -> Vec<World> {
    let base: World = vec![None; groups.len()];
    let mut varying: Vec<usize> = Vec::new();
    let mut product = 1usize;
    for (i, g) in groups.iter().enumerate() {
        if !g
            .alts
            .iter()
            .any(|a| a.flags.iter().any(|f| read.contains(f)))
        {
            continue;
        }
        let next = product.saturating_mul(g.alts.len());
        if next > MAX_WORLDS {
            continue;
        }
        product = next;
        varying.push(i);
    }
    if varying.is_empty() {
        return vec![base];
    }
    let mut worlds = vec![base];
    for &g in &varying {
        let arity = groups[g].alts.len();
        let mut next = Vec::with_capacity(worlds.len() * arity);
        for w in &worlds {
            for a in 0..arity {
                let mut w2 = w.clone();
                w2[g] = Some(a);
                next.push(w2);
            }
        }
        worlds = next;
    }
    worlds
}

/// The finale quest and its transitive stage-4 `depends_on`, restricted to
/// `keep` (the quests that complete in the chosen world) and topologically
/// sorted. Returns `(order, cyclic)`.
fn finale_order(c: &Campaign, keep: &BTreeSet<String>) -> (Vec<String>, bool) {
    let plan = &c.quest_plan.content;
    let deps: BTreeMap<&str, Vec<&str>> = plan
        .quests
        .iter()
        .map(|q| {
            (
                q.id.as_str(),
                q.depends_on.iter().map(|d| d.as_str()).collect(),
            )
        })
        .collect();

    let mut needed: BTreeSet<&str> = BTreeSet::new();
    let mut stack = vec![plan.finale.as_str()];
    while let Some(q) = stack.pop() {
        if !keep.contains(q) || !needed.insert(q) {
            continue;
        }
        if let Some(ds) = deps.get(q) {
            stack.extend(ds.iter().copied());
        }
    }

    let mut indeg: BTreeMap<&str, usize> = needed.iter().map(|q| (*q, 0)).collect();
    for q in &needed {
        if let Some(ds) = deps.get(q) {
            for d in ds {
                if needed.contains(d) {
                    *indeg.get_mut(q).unwrap() += 1;
                }
            }
        }
    }
    let mut queue: VecDeque<&str> = indeg
        .iter()
        .filter(|&(_, &d)| d == 0)
        .map(|(q, _)| *q)
        .collect();
    let mut order = Vec::new();
    while let Some(q) = queue.pop_front() {
        order.push(q.to_string());
        for r in &needed {
            if deps.get(r).is_some_and(|ds| ds.contains(&q)) {
                let e = indeg.get_mut(*r).unwrap();
                *e -= 1;
                if *e == 0 {
                    queue.push_back(r);
                }
            }
        }
    }
    let cyclic = order.len() != needed.len();
    (order, cyclic)
}

/// Order a quest's objectives by their intra-quest `after` DAG (Kahn); a cycle
/// (rejected elsewhere by `DW0140`) falls back to declaration order.
pub fn objectives_in_order(objectives: &[Objective]) -> Vec<&Objective> {
    let ids: Vec<&str> = objectives.iter().map(|o| o.id().as_str()).collect();
    let mut indeg: BTreeMap<&str, usize> = ids.iter().map(|i| (*i, 0)).collect();
    for o in objectives {
        for a in o.after() {
            if indeg.contains_key(a.as_str()) {
                *indeg.get_mut(o.id().as_str()).unwrap() += 1;
            }
        }
    }
    let mut queue: VecDeque<&str> = ids.iter().filter(|i| indeg[**i] == 0).copied().collect();
    let by_id: BTreeMap<&str, &Objective> =
        objectives.iter().map(|o| (o.id().as_str(), o)).collect();
    let mut order = Vec::new();
    while let Some(id) = queue.pop_front() {
        order.push(by_id[id]);
        for o in objectives {
            if o.after().iter().any(|a| a.as_str() == id) {
                let e = indeg.get_mut(o.id().as_str()).unwrap();
                *e -= 1;
                if *e == 0 {
                    queue.push_back(o.id().as_str());
                }
            }
        }
    }
    if order.len() != objectives.len() {
        return objectives.iter().collect();
    }
    order
}
