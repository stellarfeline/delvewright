//! The forced walk: the critical path, its steps and how it is built.

use super::*;

mod ancestors;
mod drive;
mod region;
mod triggers;

pub(in crate::compiler::plan) use ancestors::*;
pub use region::*;
pub(crate) use triggers::*;

/// A critical-path step (mirrors the amended `critical-path.json` shape).
///
/// Every step that stands for a DSL objective carries that objective's id
/// (`objective_id`, exported as the step's `objective` field). It is the step's
/// **proof obligation**: the harness passes the step only when the anchored
/// completion marker for exactly this objective arrives ([`marker_line`]). Without
/// it a step could only be checked positionally — arriving somewhere is not
/// completing anything — which is how a run once passed 22/22 on a path whose
/// campaign had in fact completed at step 12.
#[derive(Clone)]
pub enum Step {
    /// Select a class by chatting `command`.
    SelectClass {
        /// Class id.
        class_id: String,
        /// The chat command the bot sends.
        command: String,
    },
    /// Talk to an NPC; `command` fires the objective-completing option.
    TalkTo {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// NPC id.
        npc_id: String,
        /// Absolute NPC position.
        pos: [i32; 3],
        /// The chat command the bot sends.
        command: String,
    },
    /// Walk into the objective's completion volume at `pos`.
    Reach {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Anchor id.
        anchor_id: String,
        /// Absolute anchor position.
        pos: [i32; 3],
        /// Completion radius, as authored.
        radius: u32,
        /// The volume the datapack adjudicates in ([`reach_completion`]) — the
        /// same value the tick line is formatted from, carried here so the harness
        /// navigates into the server's region instead of re-deriving one.
        completion: ReachCompletion,
    },
    /// Slay a wave: goto `pos` (the wave anchor), attack entities tagged `tag`
    /// until the marker channel reports completion (v0.3).
    Kill {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Wave id (`wave/…`).
        wave_id: String,
        /// Absolute wave-anchor position.
        pos: [i32; 3],
        /// Entity tag on the wave's mobs (`dw_wave_<wave>`).
        tag: String,
        /// Total mob count.
        count: i32,
    },
    /// Collect `count` of `item` from a chest at `pos` (v0.3) — or, when
    /// `dropped` is set, off the ground where that wave died (DSL v0.9).
    Collect {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Vanilla item id.
        item: String,
        /// Required count.
        count: i32,
        /// Absolute chest-anchor position — or, for a dropped collect, the wave
        /// anchor whose floor the item lands on.
        pos: [i32; 3],
        /// The wave whose declared drop provides the item (DSL v0.9), when the
        /// objective is drop-gated. There is no container at `pos`: the harness
        /// walks the fight's ground and waits for the pickup instead of opening
        /// a block that is not there.
        dropped: Option<String>,
    },
    /// Interact at `pos`: goto, then chat `command` (the same `/trigger` the
    /// interaction advancement fires). `requires_item` gates completion (v0.3).
    Interact {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Interact anchor id.
        anchor_id: String,
        /// Absolute interact-anchor position.
        pos: [i32; 3],
        /// The chat command the bot sends.
        command: String,
        /// Item required in inventory, if any.
        requires_item: Option<String>,
        /// **The vanilla block the act uses** (spec-0093 §6.5): the objective's
        /// `prop` when it is a lever or a button, which the bot right-clicks
        /// instead of chatting `command`. `None` = the hitbox and the chat.
        block: Option<String>,
    },
    /// Perform an environment trigger the path depends on: do to its target
    /// what a player does — strike it, use it, walk within `range` of it, or
    /// strike the NPC it watches — and wait for its fired marker — [`marker_line`] with the
    /// trigger's own `trigger/<kebab>` id as the token.
    ///
    /// The one step that proves no objective and still stands somewhere. It
    /// exists because a trigger's effects can open the way on (`open-gate`,
    /// `clear-region`, `open-way`) or set a flag a later step reads, and nothing
    /// on the quest DAG makes anybody fire it: a path without this step is a walk
    /// whose proof credited a door nobody opened.
    Trigger {
        /// The `trigger/<id>` performed.
        trigger_id: String,
        /// The event, as its kebab tag (`strike` / `use` / `approach` /
        /// `strike-npc` / `strike-assembly`) — what the harness does.
        on: &'static str,
        /// The watched anchor (absent for `strike-npc` and `strike-assembly`).
        anchor_id: Option<String>,
        /// The watched NPC (`strike-npc` only).
        npc_id: Option<String>,
        /// The struck assembly (`strike-assembly` only, spec-0082).
        assembly_id: Option<String>,
        /// The cell the target stands on: the anchor, the NPC's body at this
        /// beat, or an assembly's mark.
        pos: [i32; 3],
        /// An `approach` trigger's radius; `None` for a click.
        range: Option<u32>,
        /// **Where the party stands to perform a link** (spec-0083 §3.2): a cell
        /// inside the link's `from` volume the act reaches the body from. `Some`
        /// exactly when performing this trigger carries the party — the step's
        /// `transport` marker is then the link's `to`. `None` for every trigger
        /// the path performs for its openings or its flags alone.
        stand: Option<[i32; 3]>,
        /// **The vanilla block the act uses** (spec-0093 §6.5): the trigger's
        /// `prop` when it is a lever or a button, which the bot right-clicks
        /// instead of a hitbox. `None` = the hitbox.
        block: Option<String>,
    },
    /// **Exercise a loop** (spec-0086 §5.2): walk to `pos` on the approach, cross
    /// the slab at `cross`, be moved by exactly `offset`, and repeat `times`
    /// times. Spliced in front of the first step whose leg crosses a holding
    /// slab; the party goes on from `transport`, the landing, where every
    /// crossing puts it down.
    Loop {
        /// The `loop/<id>` exercised.
        loop_id: String,
        /// A standable cell on the approach, inside the span, on the route.
        pos: [i32; 3],
        /// The slab cell the route passes through.
        cross: [i32; 3],
        /// The loop's offset `d`.
        offset: [i32; 3],
        /// How many crossings the step makes.
        times: u32,
        /// `cross + offset`: where every crossing lands the body.
        transport: [i32; 3],
    },
    /// Assert a scoreboard objective value.
    AssertComplete {
        /// The objective (`dw.campaign`).
        objective: String,
        /// Expected value.
        value: i32,
    },
}

impl Step {
    /// The `obj/<id>` this step proves, when it stands for a DSL objective.
    ///
    /// `None` for the two path-frame steps (`select-class`, `assert-complete`),
    /// which prove no objective of their own.
    pub fn objective(&self) -> Option<&str> {
        match self {
            Step::TalkTo { objective_id, .. }
            | Step::Reach { objective_id, .. }
            | Step::Kill { objective_id, .. }
            | Step::Collect { objective_id, .. }
            | Step::Interact { objective_id, .. } => Some(objective_id.as_str()),
            Step::SelectClass { .. }
            | Step::AssertComplete { .. }
            | Step::Trigger { .. }
            | Step::Loop { .. } => None,
        }
    }

    /// The absolute cell this step names, when it names one.
    ///
    /// `None` for the two path-frame steps, which stand nowhere: a class is
    /// chosen from wherever the party is, and the completion assertion is a
    /// scoreboard read. Every other step is a place the party is required to be,
    /// which is what makes this the cell a world proof can hold against the
    /// ground it put there.
    pub fn pos(&self) -> Option<[i32; 3]> {
        match self {
            Step::TalkTo { pos, .. }
            | Step::Reach { pos, .. }
            | Step::Kill { pos, .. }
            | Step::Collect { pos, .. }
            | Step::Interact { pos, .. }
            | Step::Trigger { pos, .. }
            | Step::Loop { pos, .. } => Some(*pos),
            Step::SelectClass { .. } | Step::AssertComplete { .. } => None,
        }
    }

    /// The `trigger/<id>` this step performs, when it is a trigger step.
    pub fn trigger(&self) -> Option<&str> {
        match self {
            Step::Trigger { trigger_id, .. } => Some(trigger_id.as_str()),
            _ => None,
        }
    }

    /// The cell the party stands on to perform a **link** (spec-0083 §3.2) —
    /// `Some` exactly when performing this step carries the party.
    pub fn stand(&self) -> Option<[i32; 3]> {
        match self {
            Step::Trigger { stand, .. } => *stand,
            _ => None,
        }
    }
}

/// **Does the carry that ends step `i - 1` complete step `i`?** — the reach a
/// landing puts the party inside.
///
/// A carried step's `transport` is where the party is put down before step `i`
/// begins: a crossing's entry point, a link's `to`, a loop's landing. When step
/// `i` is a `reach` whose completion volume holds that landing, the server
/// completes the objective on arrival, during step `i - 1`, and nothing is left
/// for step `i` to walk. A sealed room reached only by a ferry is the shape: its
/// beat stands where the ferry lands. The path keeps the step (every proof and
/// every step index reads it), and the export says it completes on the landing,
/// so the bot asserts the objective there instead of finding the delve finished
/// one step early. `walked` and `transports` are one path's aligned vectors —
/// the exported path's or a branch's.
pub fn completed_on_landing(walked: &[Step], transports: &[Option<[i32; 3]>], i: usize) -> bool {
    let Some(Step::Reach { completion, .. }) = walked.get(i) else {
        return false;
    };
    let Some(prev) = i.checked_sub(1) else {
        return false;
    };
    let (Some(carrier), Some(Some(landing))) = (walked.get(prev), transports.get(prev)) else {
        return false;
    };
    // A loop moves the body by an offset from wherever it crossed; every other
    // carry puts it on a fixed point.
    let exact = !matches!(carrier, Step::Loop { .. });
    completion.completes_on_landing(*landing, exact)
}

/// Version of the `critical-path.json` **contract** (its `format_version` field),
/// independent of the campaign's DSL version: the DSL describes the delve, this
/// describes what the harness is told about proving it.
///
/// * `1` — the pre-oracle shape (never written; a file with no `format_version`).
///   Steps carried no objective id, so the harness could only check position and a
///   single unanchored campaign-completion substring — a step could pass without
///   its objective completing.
/// * `2` — every objective-bearing step carries `objective`, and completion is
///   proved by the anchored per-objective marker channel ([`marker_line`]).
/// * `3` — a `reach` step carries `completion`, the volume the datapack
///   adjudicates in ([`ReachCompletion`]). The harness derives its walk goal from
///   that and no longer from the authored `radius`, which the datapack had
///   stopped reading at DSL v0.3 without telling anyone. The field is required in
///   both directions, which is what makes this a format change rather than an
///   addition: a format-2 artifact cannot tell a current bot where the objective
///   completes, and a format-2 bot would refuse the new key outright.
/// * `4` — the path carries `non_combatants`, the delve's own statement of which
///   entity kinds are never a combat target ([`crate::compiler::combat::non_combatants`]).
///   A format change rather than an addition for the same reason as `3`: a bot
///   handed a path without it would have to fall back to a literal set of entity
///   names, which is the compiler's knowledge written down where it cannot be
///   right.
///
/// The harness **requires** the current version: an older `critical-path.json`
/// (which it cannot verify) is rejected rather than run hollow.
pub const CRITICAL_PATH_FORMAT_VERSION: u32 = 4;

impl<'a> Plan<'a> {
    /// The EXECUTABLE critical path of one enumerated branch (spec-0025 §3).
    ///
    /// The same [`build_critical_path`] the exported `critical-path.json` is made
    /// of, driven by the playthrough of the world that realizes a branch instead
    /// of the default one. That identity is the point: a branch run must walk
    /// steps of exactly the shape the ladder already proves, or "branch coverage"
    /// would mean coverage of a second, less-tested contract. The branch's
    /// **scripted dialogue choices are inside the result** — each `talk-to` step
    /// carries the `/trigger` line of the option that belongs to THIS branch,
    /// which is the only player-legal way to actuate a server-driven dialog
    /// button (mineflayer cannot click one).
    ///
    /// Not called for an unreachable branch: there is no world to walk, and
    /// `DW0482` has already failed the build.
    ///
    /// `flow` is the model `path` came out of: the builder reads the flag state
    /// this branch holds at each step from its journal, which is how a `talk-to`
    /// step lands on the cast row THIS branch declares (`crate::compiler::cast::station`).
    pub fn branch_critical_path(
        &self,
        flow: &crate::compiler::flow::Flow<'_>,
        path: &crate::compiler::flow::Playthrough,
    ) -> Result<CriticalPath, PlanError> {
        self.branch_critical_path_linked(flow, path, &LinkTakes::default())
    }

    /// [`Plan::branch_critical_path`] with the route proof's link decisions for
    /// THIS branch's path spliced in (spec-0083 §6) — the branch counterpart of
    /// [`Plan::relinked`], keyed by the branch's own unlinked step indices.
    pub fn branch_critical_path_linked(
        &self,
        flow: &crate::compiler::flow::Flow<'_>,
        path: &crate::compiler::flow::Playthrough,
        takes: &LinkTakes,
    ) -> Result<CriticalPath, PlanError> {
        build_critical_path(
            self.campaign,
            &self.anchors,
            &self.npcs,
            flow,
            path,
            self.campaign_start().as_ref(),
            PathLinks {
                links: &self.links,
                takes,
            },
        )
    }

    /// The gate/seal model of ONE branch's exported path (spec-0025):
    /// the campaign's `open-gate`/`close-gate` firings with `fire_step` indices in
    /// the **branch path's own step space**, plus the strict DAG-ancestor relation
    /// over that space — exactly the model [`Plan::build`] computes for the
    /// exported path (`region_events` / `strict_ancestor_steps`), driven by the
    /// branch's own objective→step map instead of the default playthrough's.
    ///
    /// A branch path is a *different sequence* of steps, so the default path's
    /// step indices cannot be carried across (the same trap `rest_step_index`
    /// documents for bonfires): a seal attributed through the default indices
    /// would inherit another branch's ordering. Shortcut gates are sealed from
    /// world-load (`fire_step: 0`) here for the same reason they are in
    /// [`Plan::build`] — the branch must be walkable the long way too.
    ///
    /// Deterministic: both halves are pure functions of the campaign and the
    /// branch's own `CriticalPath` (ADR-0006).
    pub fn branch_gate_model(
        &self,
        cp: &CriticalPath,
    ) -> (RegionEvents, BTreeMap<usize, BTreeSet<usize>>) {
        let mut region_events =
            collect_region_events(self.campaign, &self.anchors, &cp.firing, &self.ways);
        region_events.extend(self.shortcuts.iter().map(|sc| {
            RegionEvent::forced(sc.gate_region, RegionWrite::of_block(&sc.gate_block), 0)
        }));
        // The branch's own seals and exercise writes (spec-0086 §5.3), in the
        // branch path's own step space.
        region_events.extend(cp.loops.events.iter().cloned());
        let ancestors = compute_strict_ancestor_steps(
            self.campaign,
            &cp.firing.obj_step,
            &cp.firing.trigger_step,
            cp.steps.len(),
        );
        (
            with_loop_exercises(
                region_events_of(self.campaign, region_events, &cp.firing, &self.npcs),
                &cp.loops,
            ),
            ancestors,
        )
    }

    /// Whether a gate firing at critical-path step `g` is guaranteed to have fired
    /// before a walked leg arriving at step `s` — i.e. `g`'s objective is a strict
    /// DAG ancestor of `s`'s objective (see [`Self::strict_ancestor_steps`]), or a
    /// `trigger` step the path performs before `s`. Step `0` (class-select, and
    /// the fire step of every fill a trigger or an optional root registers) is
    /// treated as always-preceding, and an arrival past the last objective has
    /// every objective and every trigger step on the path preceding it. Drives the
    /// `close-gate` seal model in
    /// `crate::compiler::nav`.
    pub fn gate_fired_before(&self, g: usize, s: usize) -> bool {
        g == 0
            || self
                .strict_ancestor_steps
                .get(&s)
                .is_some_and(|anc| anc.contains(&g))
    }

    /// Translate a [`Self::critical_path`] index into the index the SAME step
    /// carries in the **exported** `critical-path.json`.
    ///
    /// Two coordinate systems came into existence the moment spec-0016 §1's rest
    /// splice landed: `critical_path` is the compiler's own list — what every
    /// `CheckpointPlan::fire_step`, every nav proof and every internal index
    /// means — while the exported path additionally carries one `rest` step
    /// after the beat that arms each bonfire. They drift by exactly one per
    /// bonfire armed strictly earlier, and a consumer that mixed them read the
    /// wrong step (the combat plan's `step` claimed to be a `critical-path.json`
    /// index while being a `critical_path` one).
    ///
    /// **Every artifact a harness reads states EXPORTED coordinates**, and this
    /// is where that translation lives for the MAIN path.
    ///
    /// **Scope — the main `critical-path.json` only.** spec-0025's per-branch
    /// paths are a different *sequence* of the same steps, so an index cannot be
    /// carried across at all; `emit::rest_step_index` is the general translation
    /// and goes through the **objective** the arming beat names, because a fire
    /// is armed by a beat rather than by a position. On the main path that
    /// translation is the identity (an objective appears at exactly one step),
    /// which is precisely what makes the count below correct here and nowhere
    /// else. A branch-path consumer must use `rest_step_index`, never this.
    ///
    /// The arithmetic mirrors `emit::with_bonfire_rest_steps` by construction —
    /// a rest for bonfire `b` is pushed after the step at `b.fire_step`, so a
    /// step at index `i` is preceded by one rest per bonfire with
    /// `fire_step < i`. That agreement is not left to inspection:
    /// `the_combat_plan_step_indexes_the_exported_path` pins the two together
    /// against the real emitted documents (the step the plan points at must BE
    /// the encounter's kill), so a future change to the splice fails the test
    /// rather than silently desynchronising this.
    ///
    /// Identity for a campaign with no bonfire.
    pub fn exported_step(&self, step: usize) -> usize {
        step + self.bonfires().filter(|b| b.fire_step < step).count()
    }
}

/// What the path builder needs to know about links (spec-0083): every link
/// the campaign declares, and which of them the route proof took on this path.
#[derive(Clone, Copy)]
pub(crate) struct PathLinks<'l> {
    pub(super) links: &'l [crate::compiler::link::LinkPlan],
    pub(super) takes: &'l LinkTakes,
}

/// The computed critical path and its per-step metadata.
pub struct CriticalPath {
    pub steps: Vec<Step>,
    /// Per step, the links live there (spec-0083 §3.3), indices into
    /// [`Plan::links`]. Aligned 1:1 with `steps`.
    pub live_links_by_step: Vec<Vec<usize>>,
    pub(crate) transport: TransportMap,
    pub transport_by_step: Vec<Option<[i32; 3]>>,
    pub sneak_by_step: Vec<bool>,
    pub cutscene_by_step: Vec<Option<u32>>,
    /// What this path fires, and where ([`firing_of`]'s input).
    pub(crate) firing: PathFiring,
    /// The loop half of this path (spec-0086): the slab seals and exercise
    /// writes in this path's step space, and the exercise records.
    pub(crate) loops: crate::compiler::r#loop::Spliced,
}

/// **What one path fires, and at which of its steps** — everything
/// [`firing_of`] reads, for one path (the exported one, or one branch's).
#[derive(Clone, Debug, Default)]
pub(crate) struct PathFiring {
    /// Objective id → its `critical_path` step index (v0.6): roots the checkpoint
    /// no-stranding proof (DW0315) and the stealth-zone reachability proof
    /// (DW0327) at the beat that fires the effect.
    pub obj_step: BTreeMap<String, usize>,
    /// Trigger id → the `trigger` step that performs it on this path. The
    /// region-write model fires a trigger's openings at this step and at no
    /// other; a trigger absent here opens nothing any proof may lean on.
    ///
    /// **Every path act, keyed by its own id**: a loop's exercise step
    /// (spec-0086 §5.2) is keyed here by its `loop/<id>` beside the triggers,
    /// because it is the same kind of step to the ancestry — a party act no DAG
    /// orders, which precedes every step after it on this path.
    pub trigger_step: BTreeMap<String, usize>,
    /// The quests this path completes. A quest's `on_complete` fires on this
    /// path exactly when its quest is here; one absent here is never forced on
    /// the path — `obj_step` cannot answer that for a quest with no objectives.
    pub quests: BTreeSet<String>,
    /// The JSON pointer of every effect line this path fires: the flow
    /// journal's `fired` over every step, plus every line of every trigger the
    /// path performs, fired by the same replay at the state it holds there
    /// ([`crate::compiler::flow::Walk::probe`]). A line whose own gate, or an
    /// enclosing one, does not hold where the path reaches it is absent.
    pub fired: BTreeSet<String>,
    /// The members of `fired` whose gate held only on an undatable numeric term.
    /// Played by the replay; never forced.
    pub undecided: BTreeSet<String>,
    /// Each `talk-to` objective this path performs → `(npc, flat option index)`
    /// of the option it takes (spec-0088: a dialogue `set-flag` is forced only
    /// on the option the path takes).
    pub talk_taken: BTreeMap<String, (String, usize)>,
    /// Objective id → every declared datum's value as the guaranteed replay
    /// walks up to it ([`crate::compiler::flow::Walk::data`]).
    pub data_before: BTreeMap<String, BTreeMap<String, Option<i64>>>,
    /// Every declared datum's value once the path has played.
    pub data_end: BTreeMap<String, Option<i64>>,
    /// The branch flags (`branch_points[].forks_on`) this path's world never
    /// holds — the alternatives it does not take. No firing on this path sets
    /// one, so the staged-volume readings credit none of their setters.
    pub branch_excluded: BTreeSet<String>,
}

impl PathFiring {
    /// Whether this path is guaranteed to fire the line at `pointer`: the replay
    /// reached it with its whole gate decided open.
    pub fn fires(&self, pointer: &str) -> bool {
        self.fired.contains(pointer) && !self.undecided.contains(pointer)
    }
}

/// Build the critical path: select first class, then each objective of the
/// **flow-proven single-branch playthrough** ([`crate::compiler::flow::Flow::playthrough`])
/// in topological order (quests by `depends_on`, objectives by `after`), then
/// assert campaign completion. Quests that belong to a mutually exclusive branch
/// the chosen playthrough does not take are excluded, and each `talk-to` takes
/// the completing dialogue option that belongs to that branch — so the exported
/// path is a sequence one player can actually walk (proven by
/// `crate::compiler::flow::Flow::replay`, `DW0204`, before the build reaches here).
///
/// Also returns the inter-area transport map and, per step, the DSL v0.4 harness
/// hints: `sneak` (a `stealth` objective) and `cutscene_seconds` (a step whose
/// completion triggers a `Verb::Cutscene`).
///
/// `start` is where the party begins ([`resolve_campaign_start`]) — the origin
/// of the FIRST leg, and therefore part of the population every crossing is
/// decided over. `None` only for a world that resolves no entry anchor at all,
/// whose refusal is [`DW_NO_ENTRY_ANCHOR`].
pub(super) fn build_critical_path(
    campaign: &Campaign,
    anchors: &AnchorTable,
    npcs: &[NpcPlan],
    flow: &crate::compiler::flow::Flow<'_>,
    path: &crate::compiler::flow::Playthrough,
    start: Option<&(String, [i32; 3])>,
    carry: PathLinks<'_>,
) -> Result<CriticalPath, PlanError> {
    let PathLinks { links, takes } = carry;
    let mut steps = Vec::new();
    // `(path step index, first critical step it produced)`, in path order — how
    // a critical step is mapped back to the flags and data the party holds
    // walking up to it.
    let mut si_start: Vec<(usize, usize)> = Vec::new();
    // (objective id, physical area, step index) in critical-path order, for the
    // transport map and the per-step transport marker.
    let mut obj_areas: Vec<(String, String, usize)> = Vec::new();
    // Where the effect history leaves each body as each quest opens — the same
    // replay `cast::check_cast` reads, so the ledger's document arm and the
    // place arm below cannot disagree about where somebody is standing. Its
    // `At` now carries the AREA the anchor was set in, which is what makes the
    // place comparison possible at all.
    let history = crate::compiler::continuity::replay(campaign);

    // spec-0086 §5.2: the loop half of this path — the flow walk's state step
    // by step, the exercise steps it splices and the slab seals it reads off
    // the gate. Inert (and never consulted) for a campaign with no loop.
    let mut walk = flow.walk();
    let data_of = |w: &crate::compiler::flow::Walk<'_, '_>| -> BTreeMap<String, Option<i64>> {
        w.data()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    };
    let mut loops = crate::compiler::r#loop::LoopSplice::new(
        campaign,
        anchors,
        start.cloned(),
        walk.flags().clone(),
        data_of(&walk),
    );

    // select-class: first declared class.
    if let Some(first) = campaign.classes.content.classes.first() {
        steps.push(Step::SelectClass {
            class_id: first.id.as_str().to_string(),
            command: "/trigger dw.class set 1".to_string(),
        });
        loops.record();
    }

    // The branch-coherent playthrough: one world's completing quests in
    // `depends_on` order, their objectives in `after` order, and the dialogue
    // option each `talk-to` takes on that branch. Supplied by the caller so the
    // same builder serves the exported path (the default playthrough) and the
    // spec-0025 per-branch paths (the playthrough of the world realizing a
    // branch) — one code path, so a branch run walks steps built exactly like
    // the ones the ladder has always walked.
    if path.cyclic {
        return Err(PlanError::new(
            DW_BUILD,
            "internal invariant violation: a quest dependency cycle survived into critical-path \
             ordering — `DW0130` should have rejected it in validation. This is a compiler bug; \
             stop and escalate",
        ));
    }
    let stage5: BTreeMap<&str, &_> = campaign
        .quests
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q))
        .collect();

    // The flag state the party holds as it walks up to each step, and the quests
    // this playthrough ever activates. Together they are what selects a `talk-to`
    // NPC's cast row — the same journal `crate::compiler::branch`'s `DW0483` reads, so the
    // placement the ladder walks to and the placement the proofs check are chosen
    // by ONE model (see [`crate::compiler::cast::station`]).
    let flags_at: Vec<BTreeSet<String>> = flow
        .journal(path)
        .into_iter()
        .map(|s| s.flags_before)
        .collect();
    let begun: BTreeSet<String> = path.quests.iter().cloned().collect();

    // The environment triggers this path performs, keyed by the path step each
    // is performed in front of. See [`path_triggers`].
    let mut due = path_triggers(campaign, anchors, flow, path, &flags_at, &begun);
    // The presses a numeric gate owes (`DW0985`), performed after what is
    // already due at the same step; the replay is then taken over both.
    let owed = {
        let before = path_fired_lines(campaign, flow, path, &due);
        drive::drive_presses(
            campaign,
            anchors,
            path,
            &flags_at,
            &due,
            &before.data_before,
        )?
    };
    for (si, presses) in owed {
        due.entry(si).or_default().extend(presses);
    }
    let replay = path_fired_lines(campaign, flow, path, &due);
    let held_at_end: BTreeSet<String> = flow
        .journal(path)
        .last()
        .map(|s| s.flags_after.clone())
        .unwrap_or_default();
    let branch_excluded: BTreeSet<String> = campaign
        .quest_plan
        .content
        .branch_points
        .iter()
        .flat_map(|b| b.forks_on.iter().map(|f| f.as_str().to_string()))
        .filter(|f| !held_at_end.contains(f))
        .collect();
    let talk_taken: BTreeMap<String, (String, usize)> = path
        .steps
        .iter()
        .filter_map(|st| {
            let n = st.talk_option?;
            let npc = objective_quest(campaign, &st.objective).and_then(|(_, o)| match o {
                delvewright_dsl::Objective::TalkTo { npc, .. } => Some(npc.as_str().to_string()),
                _ => None,
            })?;
            Some((st.objective.clone(), (npc, n)))
        })
        .collect();
    let mut trigger_step: BTreeMap<String, usize> = BTreeMap::new();

    for (si, st) in path.steps.iter().enumerate() {
        si_start.push((si, steps.len()));
        let qid = st.quest.as_str();
        let Some(quest) = stage5.get(qid) else {
            walk.take(st);
            loops.advance(walk.flags().clone(), data_of(&walk));
            continue;
        };
        let area = campaign
            .quest_plan
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == qid)
            .map(|q| q.area.as_str())
            .unwrap_or("");
        let entry = match anchors.entry_anchor(area) {
            Some(ResolvedAnchor::Point { pos, .. }) => Some(*pos),
            _ => None,
        };
        for step in due.get(&si).into_iter().flatten() {
            if let Some(pos) = step.pos()
                && !loops.is_empty()
            {
                loops.before(&mut steps, &mut trigger_step, pos, area, entry, anchors);
            }
            if let Step::Trigger { trigger_id, .. } = step {
                trigger_step.insert(trigger_id.clone(), steps.len());
            }
            steps.push(step.clone());
            loops.record();
            if let Some(pos) = step.pos() {
                loops.stand(area, pos);
            }
        }
        let Some(obj) = quest
            .objectives
            .iter()
            .find(|o| o.id().as_str() == st.objective)
        else {
            walk.take(st);
            loops.advance(walk.flags().clone(), data_of(&walk));
            continue;
        };
        let before = steps.len();
        {
            match obj {
                Objective::TalkTo { id, npc, .. } => {
                    let npc_plan =
                        npcs.iter()
                            .find(|n| n.npc_id == npc.as_str())
                            .ok_or_else(|| {
                                PlanError::new(
                                    DW_BUILD,
                                    format!(
                                        "internal invariant violation: `talk-to` references npc \
                                         `{npc}` with no build-time plan — `DW0112`/`DW0152` \
                                         should have caught this in validation. This is a compiler \
                                         bug; stop and escalate"
                                    ),
                                )
                            })?;
                    // The branch-consistent completing option (the flow model
                    // picked it); fall back to the first completing option only
                    // for a campaign with no branch at all.
                    let opt = st
                        .talk_option
                        .and_then(|n| npc_plan.options.iter().find(|o| o.n as usize == n))
                        .or_else(|| {
                            npc_plan
                                .options
                                .iter()
                                .find(|o| o.completes.iter().any(|c| c == id.as_str()))
                        })
                        .ok_or_else(|| {
                            PlanError::new(DW_BUILD, format!(
                                "internal invariant violation: objective `{id}` has no dialogue \
                                 option completing it at build time — `DW0123`/`DW0203` should \
                                 have caught this in validation/analysis. This is a compiler bug; \
                                 stop and escalate"
                            ))
                        })?;
                    // NPC position: where the CAST LEDGER stations the body for
                    // THIS beat, on THIS path — not the stage-2 anchor.
                    //
                    // The stage-2 anchor is only where the NPC is first summoned;
                    // a `move-npc` walks him away from it and the ledger records
                    // where he then stands (`DW0461` proves the record equals the
                    // effect history). Reading the anchor here made the bot
                    // contract a second, staler source of truth: on the island,
                    // `npc/perimedes` is declared at `anchor/mouth` and cast at
                    // `anchor/alcove-2` for his stone beat, and the eye-ray bot
                    // walked to the mouth — where the sealed boulder region's
                    // wall of interaction entities stands — and could not acquire
                    // him. The emitted cast was right the whole time.
                    let decl = campaign
                        .npcs
                        .content
                        .npcs
                        .iter()
                        .find(|nn| nn.id.as_str() == npc.as_str());
                    let home_area = decl.map(|nn| nn.area.as_str()).unwrap_or(area);
                    let (npc_area, pos) = match crate::compiler::cast::station(
                        campaign,
                        npc.as_str(),
                        qid,
                        &begun,
                        flags_at.get(si).unwrap_or(&BTreeSet::new()),
                    ) {
                        Some(crate::compiler::cast::Station::At(anchor, ledger_offset)) => {
                            match body_station(
                                anchors,
                                BodyScope::Beat {
                                    beat: area,
                                    home: home_area,
                                },
                                anchor,
                            ) {
                                station @ BodyStation::At { .. } => {
                                    let (a, pos) = station
                                        .place()
                                        .map(|(a, p)| {
                                            (
                                                a.to_string(),
                                                delvewright_dsl::offset_cell(p, ledger_offset),
                                            )
                                        })
                                        .expect("an `At` station has a place");
                                    // `DW0461`, the place arm. This is the ONE
                                    // site in the compiler that reads a ledger
                                    // row's position, so it is where the row is
                                    // checked against the world: the effect
                                    // history's own anchor, resolved in the area
                                    // the effect set it in, must be this same
                                    // cell of this same building. Comparing the
                                    // two NAMES cannot see it — where two areas
                                    // declare one name the strings are equal and
                                    // the places are 256 blocks apart, which is
                                    // how a body was summoned in one building
                                    // while the party was sent to another.
                                    if let Some(NpcWhere::At(staged)) = history
                                        .at_quest_start
                                        .get(qid)
                                        .and_then(|m| m.get(npc.as_str()))
                                        && let Some((ha, hp)) = body_station(
                                            anchors,
                                            BodyScope::Beat {
                                                beat: staged.area.as_str(),
                                                home: home_area,
                                            },
                                            staged.anchor.as_str(),
                                        )
                                        .place()
                                        && (ha, delvewright_dsl::offset_cell(hp, staged.offset))
                                            != (a.as_str(), pos)
                                    {
                                        let hp = delvewright_dsl::offset_cell(hp, staged.offset);
                                        let ledger_mark = delvewright_dsl::Mark {
                                            anchor: delvewright_dsl::AnchorId(anchor.to_string()),
                                            offset: ledger_offset,
                                        };
                                        // Two different marks are the document
                                        // arm's finding, in its words; two equal
                                        // marks at two places are the split.
                                        let message = if ledger_mark != staged.mark() {
                                            crate::compiler::cast::placement_contradiction(
                                                qid,
                                                npc.as_str(),
                                                &ledger_mark.display(),
                                                &staged.mark().display(),
                                            )
                                        } else {
                                            crate::compiler::cast::station_split(
                                                qid,
                                                npc.as_str(),
                                                &ledger_mark.display(),
                                                &staged.mark().display(),
                                                (a.as_str(), pos),
                                                (ha, hp),
                                            )
                                        };
                                        return Err(PlanError::new(
                                            crate::compiler::cast::DW_CAST_PLACEMENT,
                                            message,
                                        ));
                                    }
                                    (a, pos)
                                }
                                // Not a compiler bug: the campaign named a place
                                // whose name more than one building answers to,
                                // and neither the beat's area nor the NPC's home
                                // is one of them. Picking would settle it by
                                // whichever area id sorts first.
                                BodyStation::Ambiguous(areas) => {
                                    return Err(PlanError::new(
                                        crate::compiler::gates::DW_ANCHOR_AMBIGUOUS,
                                        format!(
                                            "quest `{qid}` casts npc `{npc}` at `{anchor}`, and \
                                             {n} of this campaign's areas provide that name \
                                             ({list}) — neither the area this beat plays in \
                                             (`{area}`) nor the npc's own area (`{home_area}`) is \
                                             among them, so nothing an author can see says which \
                                             building the body is standing in. The move that \
                                             costs least here is the one this campaign owns \
                                             outright: cast the npc at a name the beat's own area \
                                             (`{area}`) provides, and no binding has to change at \
                                             all. {remedy}",
                                            n = areas.len(),
                                            list = areas
                                                .iter()
                                                .map(|a| format!("`{a}`"))
                                                .collect::<Vec<_>>()
                                                .join(", "),
                                            // The same sentence `DW0857` prints, from the one
                                            // writer: what an author may do about a name two of
                                            // their buildings answer to is a fact about anchor
                                            // names, not about the verb that said one. This site
                                            // knows the areas but not which piece of each
                                            // provides the name — `AnchorTable` records
                                            // `(area, name) -> position` and no carrier — so it
                                            // passes the areas with no pieces and the writer
                                            // leaves that clause out.
                                            remedy =
                                                crate::compiler::gates::anchor_ambiguity_remedy(
                                                    &areas
                                                        .iter()
                                                        .map(|a| {
                                                            ((*a).to_string(), BTreeSet::new())
                                                        })
                                                        .collect(),
                                                ),
                                        ),
                                    ));
                                }
                                BodyStation::Missing => {
                                    return Err(PlanError::new(
                                        DW_BUILD,
                                        format!(
                                            "internal invariant violation: quest `{qid}` casts \
                                     npc `{npc}` at `{anchor}`, which resolves to no world \
                                     position at build time — `DW0464` (dangling cast anchor) / \
                                     `DW0142` should have named it in validation. This is a \
                                     compiler bug; stop and escalate"
                                        ),
                                    ));
                                }
                            }
                        }
                        Some(crate::compiler::cast::Station::Absent(kind)) => {
                            return Err(PlanError::new(
                                DW_BUILD,
                                format!(
                                    "internal invariant violation: `talk-to` objective `{id}` needs a \
                                 body to click, but quest `{qid}`'s cast ledger declares npc \
                                 `{npc}` `\"{}\"` for this beat — `DW0195` (talk-to on an NPC a \
                                 prerequisite despawned) / `DW0461` (a declared absence that \
                                 contradicts the effect history) should have refused this in \
                                 validation. This is a compiler bug; stop and escalate",
                                    kind.token()
                                ),
                            ));
                        }
                        // No ledger row anywhere up to this beat: a pre-0.7
                        // campaign. Keep the stage-2 anchor, byte for byte.
                        None => {
                            let anchor = decl.map(|nn| nn.anchor.as_str()).unwrap_or("");
                            let offset = decl.map(|nn| nn.offset).unwrap_or([0, 0, 0]);
                            (
                                home_area.to_string(),
                                delvewright_dsl::offset_cell(
                                    point_of(anchors, home_area, anchor)?,
                                    offset,
                                ),
                            )
                        }
                    };
                    steps.push(Step::TalkTo {
                        objective_id: id.as_str().to_string(),
                        npc_id: npc.as_str().to_string(),
                        pos,
                        command: format!("/trigger {} set {}", npc_plan.trigger_objective, opt.n),
                    });
                    obj_areas.push((id.as_str().to_string(), npc_area, steps.len() - 1));
                }
                Objective::ReachAnchor {
                    id, anchor, radius, ..
                } => {
                    let pos = point_of(anchors, area, anchor.as_str())?;
                    steps.push(Step::Reach {
                        objective_id: id.as_str().to_string(),
                        anchor_id: anchor.as_str().to_string(),
                        pos,
                        radius: *radius,
                        completion: reach_completion(pos, *radius),
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
                Objective::Kill { id, wave, .. } => {
                    let w = wave_of(campaign, wave.as_str()).ok_or_else(|| {
                        PlanError::new(
                            DW_BUILD,
                            format!(
                                "internal invariant violation: `kill` objective references wave \
                                 `{wave}` with no declaration at build time — `DW0170` should have \
                                 caught this in validation. This is a compiler bug; stop and \
                                 escalate"
                            ),
                        )
                    })?;
                    let pos = point_of(anchors, area, w.anchor.as_str())?;
                    steps.push(Step::Kill {
                        objective_id: id.as_str().to_string(),
                        wave_id: wave.as_str().to_string(),
                        pos,
                        tag: wave_tag(wave.as_str()),
                        count: wave_total(w),
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
                Objective::Collect {
                    id,
                    item,
                    count,
                    anchor,
                    container,
                    dropped_by,
                    ..
                } => {
                    // The step position is the CONTAINER the bot opens: the
                    // adopted prefab chest/barrel when the objective declares one
                    // (DSL v0.8), else the chest the compiler places at `anchor`.
                    // The harness walks to this cell and opens the block standing
                    // there, so pointing it at the objective anchor while the items
                    // sit in a barrel three blocks away is a guaranteed bot stall.
                    // An unresolvable container anchor falls back to the objective
                    // anchor; the DSL tier reports it (`DW0142`).
                    // v0.9: a drop-gated collect has no container at
                    // all — the item is on the floor the wave died on, so the
                    // step points at that wave's own anchor.
                    let dropped_at = dropped_by.as_ref().and_then(|w| {
                        campaign
                            .quests
                            .content
                            .waves
                            .iter()
                            .find(|wv| wv.id.as_str() == w.as_str())
                            .and_then(|wv| point_any(anchors, wv.anchor.as_str()))
                    });
                    let pos = match dropped_at.or_else(|| {
                        container
                            .as_ref()
                            .and_then(|cont| point_any(anchors, cont.as_str()))
                    }) {
                        Some(cell) => cell,
                        None => point_of(anchors, area, anchor.as_str())?,
                    };
                    steps.push(Step::Collect {
                        objective_id: id.as_str().to_string(),
                        item: item.clone(),
                        count: *count as i32,
                        pos,
                        dropped: dropped_by.as_ref().map(|w| w.as_str().to_string()),
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
                Objective::Interact {
                    id,
                    anchor,
                    requires_item,
                    ..
                } => {
                    let pos = point_of(anchors, area, anchor.as_str())?;
                    steps.push(Step::Interact {
                        objective_id: id.as_str().to_string(),
                        anchor_id: anchor.as_str().to_string(),
                        pos,
                        command: format!("/trigger {} set 1", interact_trigger(id.as_str())),
                        requires_item: requires_item.clone(),
                        block: crate::compiler::pressable::interact_block(obj).map(str::to_string),
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
            }
        }
        // The objective step is pushed; a leg into it that crosses a holding
        // slab is exercised in front of it (spec-0086 §5.2), so it is lifted
        // off, the exercise spliced, and put back.
        walk.take(st);
        if steps.len() == before + 1 {
            let step = steps.pop().expect("the objective step was just pushed");
            let step_area = obj_areas
                .last()
                .filter(|(_, _, idx)| *idx == before)
                .map(|(_, a, _)| a.clone())
                .unwrap_or_else(|| area.to_string());
            if let Some(pos) = step.pos()
                && !loops.is_empty()
            {
                let entry = match anchors.entry_anchor(&step_area) {
                    Some(ResolvedAnchor::Point { pos, .. }) => Some(*pos),
                    _ => None,
                };
                loops.before(
                    &mut steps,
                    &mut trigger_step,
                    pos,
                    &step_area,
                    entry,
                    anchors,
                );
            }
            let pos = step.pos();
            steps.push(step);
            if let Some(last) = obj_areas.last_mut()
                && last.2 == before
            {
                last.2 = steps.len() - 1;
            }
            loops.advance(walk.flags().clone(), data_of(&walk));
            loops.record();
            if let Some(pos) = pos {
                loops.stand(&step_area, pos);
            }
        } else {
            loops.advance(walk.flags().clone(), data_of(&walk));
        }
    }

    steps.push(Step::AssertComplete {
        objective: "dw.campaign".to_string(),
        value: 1,
    });
    loops.record();
    let mut spliced = loops.finish();

    // ---- the legs the party must cross ----
    //
    // A **leg** is a move from where the party stands to where the next critical
    // objective stands, and the FIRST one begins at the campaign spawn. This is
    // the population; everything about a leg is decided here, once.
    //
    // It used to be `obj_areas.windows(2)` — every adjacent pair of objectives —
    // which is a strictly smaller set: the spawn is a leg's ORIGIN and is not an
    // objective, so the party's very first move was in no pair. It got no
    // transport, and the walkability proof (which walks the same population)
    // never examined it either. A campaign whose first objective stood in
    // another area therefore compiled clean, passed every game test, and
    // stranded the party at the spawn.
    //
    // A leg that changes area is a CROSSING, never a walk — areas sit
    // `AREA_SPACING` blocks apart across void. A crossing needs two things, and
    // a leg missing either is refused here rather than handed on to the walk
    // proof, which would report a true sentence about a route nobody was going
    // to walk:
    //   * somewhere to arrive — the destination area's entry point (`DW0872`);
    //   * something to ride — the completion of the objective the party leaves
    //     from, which the spawn cannot supply (`DW0873`).
    // Every other leg is a walk, and `DW0311` proves it over the geometry.
    let mut transport: BTreeMap<String, [i32; 3]> = BTreeMap::new();
    // Per-step transport marker, aligned with `steps`. Filled from `transport` via
    // each objective's recorded step index (gap 8).
    let mut transport_by_step: Vec<Option<[i32; 3]>> = vec![None; steps.len()];
    // Where the party stands as it sets off for the next objective: the area,
    // and the objective whose completion can carry it out of there — `None` at
    // the campaign spawn, which is the whole of `DW0873`. `None` overall when
    // the world resolves no entry anchor at all: that campaign has no start to
    // measure a first leg from and `DW0345` is its refusal, not this one.
    let mut from: Option<(&str, Option<(&str, usize)>)> =
        start.as_ref().map(|(area, _)| (area.as_str(), None));
    for (id, area, idx) in &obj_areas {
        if let Some((prev_area, carrier)) = from
            && prev_area != area.as_str()
        {
            let Some((prev_id, prev_idx)) = carrier else {
                let (start_area, start_pos) = start.as_ref().expect("a leg has an origin");
                return Err(PlanError::new(
                    DW_SPAWN_LEG_CROSSES,
                    format!(
                        "the party begins this delve in area `{start_area}`, at \
                         [{sx}, {sy}, {sz}], and the first thing the critical path asks of \
                         them — objective `{id}` — stands in area `{area}`. Areas sit \
                         {sp} blocks apart across the void with no walkable link, so that \
                         is a crossing rather than a walk, and a crossing is carried by \
                         the completion of the objective the party leaves from. At the \
                         spawn they have completed nothing, so this one can be neither \
                         ridden nor walked and the delve cannot be started. Put the \
                         campaign's first beat where the party starts — set that quest's \
                         `area` to `{start_area}` in the quest plan — or start the delve \
                         where the beat already is, by listing `{area}` before \
                         `{start_area}` in `world.areas`; the delve starts in the first \
                         area that declares an entry point of its own, so `{area}` needs \
                         an anchor carrying `\"role\": \"{role}\"` to be that area",
                        sx = start_pos[0],
                        sy = start_pos[1],
                        sz = start_pos[2],
                        sp = AREA_SPACING,
                        role = AnchorRole::Entry,
                    ),
                ));
            };
            let Some(ResolvedAnchor::Point { pos, .. }) = anchors.entry_anchor(area) else {
                return Err(PlanError::new(
                    DW_CROSSING_NO_ENTRY,
                    format!(
                        "completing objective `{prev_id}` in area `{prev_area}` carries the \
                         party across to objective `{id}` in area `{area}` — areas sit \
                         {sp} blocks apart across the void with no walkable link, so the \
                         party has to be put down inside `{area}`, and {said}. Fix it \
                         where the anchors are declared: give the piece the party arrives \
                         in an anchor at that cell and put `\"role\": \"{role}\"` on it (in \
                         a pool, that is the prefab the layout is seeded from), or bind \
                         `{area}` to a prefab that already has one",
                        sp = AREA_SPACING,
                        role = AnchorRole::Entry,
                        said = match anchors.entry_anchor_name(area) {
                            // A named entry that is not a cell: a gate anchor
                            // declares a plane, and a body cannot be put down on
                            // a plane. Saying "declares no entry point" here
                            // would be false, and the author would go looking
                            // for the declaration they can see.
                            Some(name) => format!(
                                "`{area}` declares its entry as `{name}`, which resolves to a \
                                 region rather than to a cell — a body cannot be put down on a \
                                 plane"
                            ),
                            None => format!(
                                "nothing says where: no anchor in `{area}`'s prefab carries \
                                 `\"role\": \"{role}\"`, and an anchor's name is never \
                                 consulted for this",
                                role = AnchorRole::Entry,
                            ),
                        },
                    ),
                ));
            };
            transport.insert(prev_id.to_string(), *pos);
            transport_by_step[prev_idx] = Some(*pos);
        }
        from = Some((area.as_str(), Some((id.as_str(), *idx))));
    }

    // DSL v0.4 per-step harness hint `sneak` (a stealth objective). The
    // cutscene hold is read off the finished step list ([`hold::after_holds`]).
    let mut sneak_by_step = vec![false; steps.len()];
    for (obj_id, _, step_idx) in &obj_areas {
        if let Some((_, obj)) = objective_quest(campaign, obj_id) {
            sneak_by_step[*step_idx] = obj.stealth();
        }
    }

    let mut obj_step: BTreeMap<String, usize> = obj_areas
        .iter()
        .map(|(id, _, idx)| (id.clone(), *idx))
        .collect();

    // An exercise step's transport is its landing (spec-0086 §5.2), marked on
    // the same per-step channel a crossing is, so every reader of visited
    // positions reads the leg after it as starting there.
    for (i, s) in steps.iter().enumerate() {
        if let Step::Loop { transport, .. } = s {
            transport_by_step[i] = Some(*transport);
        }
    }

    // ---- the links live at each step (spec-0083 §3.3) ----
    //
    // A link is live where the trigger's flag gate and every `when` on the way
    // to its teleport hold under the flags the party holds walking up to the
    // step, and every numeric term compares true against the writes the path
    // has performed by then — the same replay `DW0879` reads (`Flow::walk`).
    // Computed only when the campaign declares a link, so nothing else moves.
    let mut live_links_by_step: Vec<Vec<usize>> = vec![Vec::new(); steps.len()];
    if !links.is_empty() {
        let mut data_at: Vec<BTreeMap<String, Option<i64>>> = Vec::new();
        let mut walk = flow.walk();
        for st in &path.steps {
            data_at.push(
                walk.data()
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect(),
            );
            walk.take(st);
        }
        for (k, live) in live_links_by_step.iter_mut().enumerate() {
            let Some(si) = si_start
                .iter()
                .rev()
                .find(|(_, first)| *first <= k)
                .map(|(si, _)| *si)
            else {
                continue;
            };
            let (Some(held), Some(data)) = (flags_at.get(si), data_at.get(si)) else {
                continue;
            };
            *live = links
                .iter()
                .enumerate()
                .filter(|(_, l)| l.flags_open(held) && l.state_open(data))
                .map(|(i, _)| i)
                .collect();
        }
    }

    // ---- the links the route proof took (spec-0083 §3.4) ----
    //
    // A performed trigger that carries is marked where it stands; a taken link
    // is a `trigger` step spliced directly in front of the leg it carries, in
    // the order taken. Every per-step vector and every step-index map is
    // re-indexed in the same pass, so the path is one path again.
    for (&k, &(li, stand)) in &takes.performed {
        if let (Some(Step::Trigger { stand: at, .. }), Some(l)) = (steps.get_mut(k), links.get(li))
        {
            *at = Some(stand);
            transport_by_step[k] = Some(l.to);
        }
    }
    if !takes.spliced.is_empty() {
        let n = steps.len();
        let mut new_steps = Vec::with_capacity(n);
        let mut new_transport = Vec::with_capacity(n);
        let mut new_sneak = Vec::with_capacity(n);
        let mut new_live = Vec::with_capacity(n);
        let mut moved: Vec<usize> = Vec::with_capacity(n);
        let mut spliced_at: Vec<(String, usize)> = Vec::new();
        for (k, step) in steps.into_iter().enumerate() {
            for &(li, stand) in takes.spliced.get(&k).into_iter().flatten() {
                let Some(l) = links.get(li) else { continue };
                spliced_at.push((l.trigger_id.clone(), new_steps.len()));
                new_steps.push(Step::Trigger {
                    trigger_id: l.trigger_id.clone(),
                    on: l.on,
                    anchor_id: l.anchor_id.clone(),
                    npc_id: l.npc_id.clone(),
                    assembly_id: l.assembly_id.clone(),
                    pos: l.body.first().copied().unwrap_or(stand),
                    range: l.range,
                    stand: Some(stand),
                    block: pressed_block(campaign, &l.trigger_id),
                });
                new_transport.push(Some(l.to));
                new_sneak.push(false);
                new_live.push(live_links_by_step[k].clone());
            }
            moved.push(new_steps.len());
            new_steps.push(step);
            new_transport.push(transport_by_step[k]);
            new_sneak.push(sneak_by_step[k]);
            new_live.push(std::mem::take(&mut live_links_by_step[k]));
        }
        for idx in obj_step.values_mut() {
            *idx = moved[*idx];
        }
        // Every trigger step moves with its step; a spliced link records the
        // first step that performs its trigger, which is what the region model
        // roots that trigger's openings at.
        for idx in trigger_step.values_mut() {
            *idx = moved[*idx];
        }
        for (id, at) in spliced_at {
            let e = trigger_step.entry(id).or_insert(at);
            *e = (*e).min(at);
        }
        // spec-0086 × spec-0083: the loop half of this path was built in the
        // step space before any link was spliced; its seals and exercise
        // records move with their steps.
        spliced.reindex(&moved, new_steps.len());
        steps = new_steps;
        transport_by_step = new_transport;
        sneak_by_step = new_sneak;
        live_links_by_step = new_live;
    }

    // Every cutscene a step's completion schedules, at any depth of its
    // bundles, read off the step list as it is exported (links spliced).
    let cutscene_by_step = crate::compiler::hold::after_holds(campaign, &steps);
    Ok(CriticalPath {
        steps,
        live_links_by_step,
        transport,
        transport_by_step,
        sneak_by_step,
        cutscene_by_step,
        firing: PathFiring {
            obj_step,
            trigger_step,
            quests: path.quests.iter().cloned().collect(),
            fired: replay.fired,
            undecided: replay.undecided,
            talk_taken,
            data_before: replay.data_before,
            data_end: replay.data_end,
            branch_excluded,
        },
        loops: spliced,
    })
}

/// **Every effect line a path fires** — the input [`PathFiring::fires`] reads.
///
/// One walk of the replay under its **guaranteed** stance
/// ([`crate::compiler::flow::Flow::walk_performing`]): a flag an ambient producer
/// sets is held only once the path performs the trigger that owns it, and a
/// producer no trigger owns (a trap or timed-gate disarm, a purchase) is never
/// held. Each step's `fired`/`undecided` is the replay's own gate test where it
/// reaches each line — flags, and numeric terms against the value the walk holds
/// there. A trigger the path performs in front of a step (`due`) has its
/// `effects` fired by the same replay ([`crate::compiler::flow::Walk::probe`]) at
/// the state the walk holds there, and from then on its producers are credited.
fn path_fired_lines(
    campaign: &Campaign,
    flow: &crate::compiler::flow::Flow<'_>,
    path: &crate::compiler::flow::Playthrough,
    due: &BTreeMap<usize, Vec<Step>>,
) -> PathReplay {
    let mut data_before: BTreeMap<String, BTreeMap<String, Option<i64>>> = BTreeMap::new();
    let mut fired: BTreeSet<String> = BTreeSet::new();
    let mut undecided: BTreeSet<String> = BTreeSet::new();
    // A trigger's effect list and its root pointer, from the one root walk.
    let mut roots: BTreeMap<&str, (String, &[QuestEffect])> = BTreeMap::new();
    for_each_effect_root(campaign, &mut |site, list| {
        if let EffectRoot::Trigger(t) = site.root {
            roots.insert(t.id.as_str(), (site.path.clone(), list));
        }
    });
    let mut walk = flow.walk_performing();
    for (si, step) in path.steps.iter().enumerate() {
        for performed in due.get(&si).into_iter().flatten() {
            let Step::Trigger { trigger_id, .. } = performed else {
                continue;
            };
            if let Some((base, effs)) = roots.get(trigger_id.as_str()) {
                let (f, u) = walk.probe(trigger_id, effs, base);
                fired.extend(f);
                undecided.extend(u);
            }
            walk.perform(trigger_id);
        }
        data_before.insert(step.objective.clone(), data_now(&walk));
        let taken = walk.take(step);
        fired.extend(taken.fired);
        undecided.extend(taken.undecided);
    }
    PathReplay {
        fired,
        undecided,
        data_before,
        data_end: data_now(&walk),
    }
}

/// What [`path_fired_lines`] reads off one walk of the guaranteed replay.
struct PathReplay {
    fired: BTreeSet<String>,
    undecided: BTreeSet<String>,
    data_before: BTreeMap<String, BTreeMap<String, Option<i64>>>,
    data_end: BTreeMap<String, Option<i64>>,
}

/// Every declared datum's value the walk holds now, by id.
fn data_now(walk: &crate::compiler::flow::Walk<'_, '_>) -> BTreeMap<String, Option<i64>> {
    walk.data()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
}
