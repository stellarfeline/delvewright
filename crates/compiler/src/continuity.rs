//! NPC location-continuity lint (`DW0351`, warning tier).
//!
//! Owner-observed defects (island QA round 6): `npc/perimedes` — anchored deep
//! inside the cave, deferred-spawned mid-story having never been staged entering
//! — pops into existence where players may already be looking; `npc/antiphos` —
//! "grabbed at the cave mouth" by the story — actually vanishes at the beach
//! camp, where he has stood (and stayed) since world init. Both are *continuity*
//! breaks: an NPC materialized or vanished at a location discontinuous with
//! where the campaign last staged it, with no movement in between.
//!
//! This lint tracks each NPC's **staged location history** through the campaign
//! timeline — the initial stage-2 anchor (or off-stage for a `deferred` NPC),
//! every `move-npc` destination, and every `despawn-npc`/`spawn-npc` pair (a
//! deferred `spawn-npc` places the NPC at its declared anchor) — and warns when:
//!
//! * **re-entry jump** — a `spawn-npc` re-materializes an NPC at its declared
//!   anchor after it was last staged somewhere else (it teleports across the
//!   map with no walk in between);
//! * **unstaged entrance** — a never-yet-staged deferred NPC materializes
//!   mid-story with no staged arrival covering the spot (the accepted staging
//!   shape is firing the `spawn-npc` from a `move-actor`/`move-npc` `on_arrive`
//!   whose destination IS the NPC's anchor — a walked hand-off);
//! * **remote dismissal** — a `despawn-npc` fires from a beat staged at one
//!   anchor while the NPC's body stands at another: the story dismisses the
//!   character *here* while it vanishes, unseen, *there*.
//!
//! ## Severity: warning, not error
//!
//! Whether a jump reads as broken is authorial judgement — narrative cover ("he
//! slipped away while you slept") can make any of these legitimate. The lint
//! names the discontinuity concretely and prescribes the remedy; author taste
//! stays in charge (`delvec` exits non-zero only on `Severity::Error`).
//!
//! ## Conservative model (no temporal reasoning)
//!
//! Locations are **symbolic anchor names** (two events are co-located iff they
//! name the same anchor — no geometry, no distances). The timeline is the
//! quest-DAG linearization: stage-4 `depends_on` topological order, objectives
//! in `after` order, each completion bundle in declared order, descending into
//! `sequence` steps and `on_arrive` bundles in place. Anything whose firing
//! time is statically unknowable makes the affected NPC **untracked** rather
//! than guessed at: an NPC touched by a lifecycle effect from an environment
//! trigger, a dialogue option, an `on_respawn`/`on_caught` reaction bundle, or
//! a flag-gated (`requires_flags`/`forbids_flags`) effect is excluded from the
//! lint entirely. No false certainty: a warning is only raised where the
//! DAG-ordered history is unambiguous.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{Campaign, Diagnostic, Objective, QuestEffect};

/// Stable code for the NPC location-continuity warning.
pub const DW_NPC_CONTINUITY: &str = "DW0351";

/// A tracked NPC's staged-location state while replaying the timeline.
struct NpcState {
    /// The stage-2 declared anchor — where every `spawn-npc` places the body.
    declared_anchor: String,
    /// `Some(anchor)` while the NPC is on stage (in the world) at that anchor.
    on_stage: Option<String>,
    /// Where the NPC last stood when it left the stage (despawn), if ever.
    last_staged: Option<String>,
    /// Whether the NPC has ever been on stage (init or a previous spawn).
    ever_staged: bool,
}

/// Where the replayed effect history leaves an NPC at some point on the
/// timeline. The vocabulary the cast ledger's placement proof checks against
/// (spec-0020 proof 2) — same conservative model as the lint: symbolic anchor
/// names, and `Indeterminate` rather than a guess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NpcWhere {
    /// On stage at this anchor.
    At(String),
    /// Not in the world (despawned, or deferred and not yet spawned).
    Offstage,
    /// The history does not determine it: this NPC's lifecycle is driven from a
    /// branch (a flag-gated effect or a dialogue option) or from a source with no
    /// static position at all (an environment trigger, a reaction bundle). The
    /// payload names the reason, for the diagnostic that reports it.
    Indeterminate(&'static str),
}

/// One full replay of the campaign timeline: the `DW0351` findings, plus a
/// snapshot of every NPC's whereabouts at the moment each quest becomes active.
pub struct Timeline {
    /// `DW0351` warnings, in timeline order.
    pub diags: Vec<Diagnostic>,
    /// `quest id -> npc id -> whereabouts as that quest opens`. The snapshot is
    /// taken *before* the quest's own effect bundles fire, so it is the state the
    /// quest's `cast` block describes.
    pub at_quest_start: BTreeMap<String, BTreeMap<String, NpcWhere>>,
}

/// Replay the campaign timeline once, collecting both the continuity findings
/// and the per-quest whereabouts snapshot.
///
/// The snapshot exists because spec-0020's cast ledger has to answer "where does
/// the effect history actually leave this NPC when this quest opens?" — a value
/// this replay has always computed and immediately overwritten. Exposing it here
/// keeps exactly one model of NPC whereabouts in the compiler: the lint and the
/// ledger proof cannot disagree about where somebody is standing.
pub fn replay(c: &Campaign) -> Timeline {
    let mut diags = Vec::new();
    let mut at_quest_start: BTreeMap<String, BTreeMap<String, NpcWhere>> = BTreeMap::new();
    let excluded = excluded_npcs(c);

    // Initial staging: non-deferred NPCs stand at their anchor from world init.
    let mut state: BTreeMap<String, NpcState> = BTreeMap::new();
    for n in &c.npcs.content.npcs {
        state.insert(
            n.id.as_str().to_string(),
            NpcState {
                declared_anchor: n.anchor.as_str().to_string(),
                on_stage: (!n.deferred).then(|| n.anchor.as_str().to_string()),
                last_staged: None,
                ever_staged: !n.deferred,
            },
        );
    }

    // Replay the quest-DAG linearization.
    for q in quests_in_dag_order(c) {
        // Snapshot before this quest's own bundles fire: this is the world the
        // quest's `cast` block declares.
        at_quest_start.insert(
            q.id.as_str().to_string(),
            state
                .iter()
                .map(|(npc, st)| {
                    let w = match excluded.get(npc.as_str()) {
                        Some(reason) => NpcWhere::Indeterminate(reason),
                        None => match &st.on_stage {
                            Some(a) => NpcWhere::At(a.clone()),
                            None => NpcWhere::Offstage,
                        },
                    };
                    (npc.clone(), w)
                })
                .collect(),
        );
        let qi = c
            .quests
            .content
            .quests
            .iter()
            .position(|x| x.id.as_str() == q.id.as_str())
            .unwrap_or(0);
        let objectives = objectives_in_after_order(&q.objectives);
        let mut last_scene: Option<String> = None;
        for obj in &objectives {
            let scene = scene_anchor(obj, &state);
            if let Some(effs) = q.on_objective_complete.get(obj.id()) {
                let path = format!(
                    "/content/quests/{qi}/on_objective_complete/{}",
                    obj.id().as_str()
                );
                walk_bundle(
                    effs,
                    &path,
                    scene.as_deref(),
                    None,
                    &excluded,
                    &mut state,
                    &mut diags,
                );
            }
            if scene.is_some() {
                last_scene = scene;
            }
        }
        walk_bundle(
            &q.on_complete,
            &format!("/content/quests/{qi}/on_complete"),
            last_scene.as_deref(),
            None,
            &excluded,
            &mut state,
            &mut diags,
        );
    }
    Timeline {
        diags,
        at_quest_start,
    }
}

/// Run the lint over the whole campaign. Returns `DW0351` warnings (in timeline
/// order). Empty for a campaign with no NPC lifecycle discontinuities.
pub fn check_npc_continuity(c: &Campaign) -> Vec<Diagnostic> {
    replay(c).diags
}

/// Walk one effect bundle in declared order, descending into `sequence` steps
/// (same scene) and `on_arrive` bundles (the enclosing move's destination
/// becomes the `covered_by_arrival_at` context). Reaction bundles
/// (`on_respawn`/`on_caught`) are NOT descended: they fire at statically
/// unknowable times, and any NPC they touch is already excluded.
#[allow(clippy::too_many_arguments)]
fn walk_bundle(
    effs: &[QuestEffect],
    path: &str,
    scene: Option<&str>,
    covered_by_arrival_at: Option<&str>,
    excluded: &BTreeMap<String, &'static str>,
    state: &mut BTreeMap<String, NpcState>,
    diags: &mut Vec<Diagnostic>,
) {
    for (j, e) in effs.iter().enumerate() {
        let epath = format!("{path}/{j}");
        match e {
            QuestEffect::Sequence { steps } => {
                for (s, step) in steps.iter().enumerate() {
                    walk_bundle(
                        &step.effects,
                        &format!("{epath}/steps/{s}/effects"),
                        scene,
                        None,
                        excluded,
                        state,
                        diags,
                    );
                }
            }
            QuestEffect::MoveActor {
                to_anchor,
                on_arrive,
                ..
            } => {
                walk_bundle(
                    on_arrive,
                    &format!("{epath}/on_arrive"),
                    Some(to_anchor.as_str()),
                    Some(to_anchor.as_str()),
                    excluded,
                    state,
                    diags,
                );
            }
            QuestEffect::MoveNpc {
                npc,
                to_anchor,
                on_arrive,
                ..
            } => {
                if !excluded.contains_key(npc.as_str())
                    && let Some(st) = state.get_mut(npc.as_str())
                    && st.on_stage.is_some()
                {
                    st.on_stage = Some(to_anchor.as_str().to_string());
                }
                walk_bundle(
                    on_arrive,
                    &format!("{epath}/on_arrive"),
                    Some(to_anchor.as_str()),
                    Some(to_anchor.as_str()),
                    excluded,
                    state,
                    diags,
                );
            }
            QuestEffect::DespawnNpc { npc, .. } => {
                if excluded.contains_key(npc.as_str()) {
                    continue;
                }
                let Some(st) = state.get_mut(npc.as_str()) else {
                    continue;
                };
                let Some(loc) = st.on_stage.take() else {
                    continue; // already off stage — an idempotent cleanup
                };
                if let Some(scene) = scene
                    && scene != loc
                {
                    diags.push(Diagnostic::warning(
                        DW_NPC_CONTINUITY,
                        "quests",
                        epath.clone(),
                        format!(
                            "npc `{npc}` is despawned while staged at `{loc}`, but the beat that \
                             removes it plays at `{scene}` — the story dismisses the character at \
                             `{scene}` while its body vanishes, unseen, at `{loc}`. Walk it into \
                             the scene first (`move-npc` to `{scene}` before this beat), despawn \
                             it from a beat staged at `{loc}`, or accept the off-screen exit with \
                             explicit narrative cover"
                        ),
                    ));
                }
                st.last_staged = Some(loc);
            }
            QuestEffect::SpawnNpc { npc, .. } => {
                if excluded.contains_key(npc.as_str()) {
                    continue;
                }
                let Some(st) = state.get_mut(npc.as_str()) else {
                    continue;
                };
                if st.on_stage.is_some() {
                    continue; // already present — spawn-npc is an idempotent no-op
                }
                let anchor = st.declared_anchor.clone();
                if st.ever_staged {
                    if let Some(last) = &st.last_staged
                        && last != &anchor
                    {
                        diags.push(Diagnostic::warning(
                            DW_NPC_CONTINUITY,
                            "quests",
                            epath.clone(),
                            format!(
                                "npc `{npc}` re-materializes at its declared anchor `{anchor}`, \
                                 but it was last staged at `{last}` — it jumps across the map \
                                 with no movement in between. Stage the move (`move-npc` it back \
                                 to `{anchor}` before the despawn), re-anchor the npc at `{last}`, \
                                 or accept the jump with explicit narrative cover"
                            ),
                        ));
                    }
                } else if covered_by_arrival_at != Some(anchor.as_str()) {
                    diags.push(Diagnostic::warning(
                        DW_NPC_CONTINUITY,
                        "quests",
                        epath.clone(),
                        format!(
                            "npc `{npc}` is deferred-spawned mid-story at `{anchor}` having never \
                             been staged entering — it pops into existence where players may \
                             already be looking, with no movement in between. Stage the entrance: \
                             fire this `spawn-npc` from a `move-actor`/`move-npc` `on_arrive` \
                             arriving at `{anchor}` (walk a stand-in to the spot and swap the npc \
                             in on arrival), spawn it at world init instead (drop `deferred`), or \
                             accept the materialization with explicit narrative cover"
                        ),
                    ));
                }
                st.on_stage = Some(anchor);
                st.ever_staged = true;
            }
            // Reaction bundles fire at unknowable times — do not descend; any NPC
            // they stage is already excluded from tracking.
            QuestEffect::SetCheckpoint { .. }
            | QuestEffect::Bonfire { .. }
            | QuestEffect::BeginStealth { .. } => {}
            _ => {}
        }
    }
}

/// NPCs the lint must NOT track: any NPC whose lifecycle
/// (`spawn-npc`/`despawn-npc`/`move-npc`) is touched from a source with no
/// static position on the quest DAG — an environment trigger, a dialogue
/// option, an `on_respawn`/`on_caught` reaction bundle — or by a flag-gated
/// effect (whether it fires depends on runtime flag state). Conservative by
/// construction: exclusion silences the lint for that NPC rather than warning
/// on a history the compiler cannot order.
/// The value is *why* — the phrase a diagnostic can drop into a sentence.
fn excluded_npcs(c: &Campaign) -> BTreeMap<String, &'static str> {
    let mut out: BTreeMap<String, &'static str> = BTreeMap::new();
    /// Record a reason, keeping the first one recorded (the walk order is
    /// deterministic, so the attributed reason is too).
    fn note(out: &mut BTreeMap<String, &'static str>, npc: &str, reason: &'static str) {
        out.entry(npc.to_string()).or_insert(reason);
    }

    /// The lifecycle-target NPC of `e`, if it is a lifecycle effect.
    fn lifecycle_npc(e: &QuestEffect) -> Option<&str> {
        match e {
            QuestEffect::SpawnNpc { npc, .. } => Some(npc.as_str()),
            QuestEffect::DespawnNpc { npc, .. } | QuestEffect::MoveNpc { npc, .. } => {
                Some(npc.as_str())
            }
            _ => None,
        }
    }

    /// Walk `effs`; `reactive` is true once inside an `on_respawn`/`on_caught`
    /// bundle (their contents are unordered w.r.t. the DAG).
    fn scan(effs: &[QuestEffect], reactive: bool, out: &mut BTreeMap<String, &'static str>) {
        for e in effs {
            if let Some(npc) = lifecycle_npc(e) {
                if reactive {
                    note(
                        out,
                        npc,
                        "its lifecycle is driven from a reaction bundle \
                         (`on_respawn`/`on_rest`/`on_caught`), which fires at no fixed point on \
                         the quest DAG",
                    );
                } else if !e.requires_flags().is_empty() || !e.forbids_flags().is_empty() {
                    note(
                        out,
                        npc,
                        "its lifecycle is driven by a flag-gated effect, so it stands in \
                         different places on different branches",
                    );
                }
            }
            match e {
                QuestEffect::SetCheckpoint { on_respawn, .. } => scan(on_respawn, true, out),
                QuestEffect::Bonfire { on_rest, .. } => scan(on_rest, true, out),
                QuestEffect::BeginStealth { on_caught, .. } => scan(on_caught, true, out),
                _ => {
                    for list in e.nested_effect_lists() {
                        scan(list, reactive, out);
                    }
                }
            }
        }
    }

    // Every effect root, inherited from the single enumeration. Which roots have a
    // DAG position is the whole question this function asks, so it is answered
    // per-root off the site rather than by re-deriving it from a second walk — and
    // a root added later gets an answer here or fails to compile.
    //
    // This walk used to name three of the five. `traps[].payload` and a dialogue
    // option's `set-checkpoint` `on_respawn` bundle are the two sources with the
    // LEAST static position of any — the party may never spring the trap and nobody
    // is forced to die — so missing them made the lint under-exclude, which is the
    // direction that WARNS on a history the compiler cannot order.
    delvewright_dsl::for_each_effect_root(c, &mut |site, effs| match site.owner {
        delvewright_dsl::EffectRootOwner::ObjectiveComplete { .. }
        | delvewright_dsl::EffectRootOwner::QuestComplete { .. } => {
            scan(effs, false, &mut out);
        }
        // No DAG position at all: the whole bundle is unordered, so every
        // lifecycle effect in it is excluded regardless of depth.
        delvewright_dsl::EffectRootOwner::Trigger(_)
        | delvewright_dsl::EffectRootOwner::TrapPayload(_)
        | delvewright_dsl::EffectRootOwner::DialogueRespawn => {
            let reason = match site.owner {
                delvewright_dsl::EffectRootOwner::Trigger(_) => {
                    "its lifecycle is driven from an environment trigger, which the \
                     player may fire at any time (or never)"
                }
                delvewright_dsl::EffectRootOwner::TrapPayload(_) => {
                    "its lifecycle is driven from a trap payload, which the party may \
                     spring at any time (or never)"
                }
                _ => {
                    "its lifecycle is driven from a dialogue option's `on_respawn` \
                     bundle, which fires only on a death nobody is forced to take"
                }
            };
            for e in effs {
                e.visit_deep(&mut |x| {
                    if let Some(npc) = lifecycle_npc(x) {
                        note(&mut out, npc, reason);
                    }
                });
            }
        }
    });
    // Dialogue-fired spawns likewise (the only dialogue lifecycle verb).
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for opt in &node.options {
                for e in &opt.effects {
                    if let Some(npc) = e.spawn_npc() {
                        note(
                            &mut out,
                            npc.as_str(),
                            "its lifecycle is driven from a dialogue option, so it stands in \
                             different places depending on what the player said",
                        );
                    }
                }
            }
        }
    }
    out
}

/// The symbolic scene anchor of an objective — where the beat that fires its
/// completion bundle plays: the objective's own anchor (`reach-anchor` /
/// `collect` / `interact`), its wave's spawn anchor (`kill`), or the target
/// NPC's currently staged anchor (`talk-to`). `None` when unknowable.
fn scene_anchor(obj: &Objective, state: &BTreeMap<String, NpcState>) -> Option<String> {
    match obj {
        Objective::ReachAnchor { anchor, .. }
        | Objective::Collect { anchor, .. }
        | Objective::Interact { anchor, .. } => Some(anchor.as_str().to_string()),
        Objective::Kill { .. } => None, // wave anchors resolve per campaign; keep symbolic
        Objective::TalkTo { npc, .. } => state.get(npc.as_str()).and_then(|s| s.on_stage.clone()),
    }
}

/// Stage-5 quests in stage-4 `depends_on` topological order (stable: stage-4
/// declaration order breaks ties); stage-5 quests absent from stage 4 (an error
/// elsewhere — `DW0151`) are appended in declaration order so the lint stays
/// total.
fn quests_in_dag_order(c: &Campaign) -> Vec<&delvewright_dsl::Quest> {
    let plan = &c.quest_plan.content.quests;
    let ids: Vec<&str> = plan.iter().map(|q| q.id.as_str()).collect();
    let index: BTreeMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut indegree: Vec<usize> = vec![0; plan.len()];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); plan.len()];
    for (i, q) in plan.iter().enumerate() {
        for dep in &q.depends_on {
            if let Some(&d) = index.get(dep.as_str()) {
                indegree[i] += 1;
                dependents[d].push(i);
            }
        }
    }
    let mut order: Vec<usize> = Vec::with_capacity(plan.len());
    let mut ready: Vec<usize> = (0..plan.len()).filter(|&i| indegree[i] == 0).collect();
    while let Some(&i) = ready.first() {
        ready.remove(0);
        order.push(i);
        for &dep in &dependents[i] {
            indegree[dep] -= 1;
            if indegree[dep] == 0 {
                // keep declaration order among newly-ready quests
                let pos = ready.partition_point(|&r| r < dep);
                ready.insert(pos, dep);
            }
        }
    }
    let mut out: Vec<&delvewright_dsl::Quest> = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for i in order {
        let id = ids[i];
        if let Some(q) = c.quests.content.quests.iter().find(|q| q.id.as_str() == id) {
            out.push(q);
            seen.insert(id);
        }
    }
    for q in &c.quests.content.quests {
        if !seen.contains(q.id.as_str()) {
            out.push(q);
        }
    }
    out
}

/// Objectives in `after` topological order (stable: declaration order breaks
/// ties; a cycle — an error elsewhere, `DW0140` — falls back to declaration
/// order for the unresolved tail so the lint stays total).
fn objectives_in_after_order(objs: &[Objective]) -> Vec<&Objective> {
    let index: BTreeMap<&str, usize> = objs
        .iter()
        .enumerate()
        .map(|(i, o)| (o.id().as_str(), i))
        .collect();
    let mut placed = vec![false; objs.len()];
    let mut out: Vec<&Objective> = Vec::with_capacity(objs.len());
    loop {
        let mut progressed = false;
        for (i, o) in objs.iter().enumerate() {
            if placed[i] {
                continue;
            }
            let ready = o
                .after()
                .iter()
                .all(|a| index.get(a.as_str()).is_none_or(|&j| placed[j]));
            if ready {
                placed[i] = true;
                out.push(o);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    for (i, o) in objs.iter().enumerate() {
        if !placed[i] {
            out.push(o);
        }
    }
    out
}
