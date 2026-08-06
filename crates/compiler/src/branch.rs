//! Branch-complete narrative verification (spec-0025, `DW0480`–`DW0485`).
//!
//! ## Why this exists
//!
//! The validation ladder proved ONE critical path. A narrative branch — a choice
//! that forks who lives, three endings — was declared in the DSL,
//! reachability-checked as a graph, and then never played. The island round-13
//! defect is the whole blind class in one shape: the flee branch's cast ledger
//! said Antiphos lives, but the staging still belonged to the death branch — an
//! NPC despawned himself, another held a cave the party had left, a third
//! mourned a man standing beside him. **The fork moved the ledger but never
//! moved the bodies**, and no check owned the gap.
//!
//! So "provably completable by machine" quantifies over **branches**, not paths.
//! This module is the compiler's half of that: branches become a first-class,
//! *verified* declaration, every existing static proof re-runs under each
//! branch's flag assignment, and the compiler compiles the DSL **back into
//! natural language** — the per-branch chronicle — so a reviewer can compare
//! like with like (the decompilation principle, spec-0025 §Ruling).
//!
//! ## The model
//!
//! Stage 4 declares its `branch_points`: the flag set a fork owns, the quest it
//! opens at, and the branches it offers. An **enumerated branch** is one point of
//! the product over the declared points — so the branch set is authored and
//! small, never a combinatorial sweep of every flag in the campaign. Each branch
//! carries a **flag assignment**: the flags it lists are pinned SET, and every
//! other flag of its points' `forks_on` is pinned UNSET. That second half is what
//! makes leakage decidable.
//!
//! An assignment is then realized against [`crate::flow`]'s enumerated worlds: a
//! world realizes a branch when its solved flag set holds every pinned-set flag
//! and no pinned-unset one. No world holding the set flags at all means the
//! branch is not reachable (`DW0482`); worlds holding them but also holding a
//! sibling's flag means the branches are not exclusive (`DW0484`).
//!
//! ## The proofs
//!
//! | Code | Proof |
//! |------|-------|
//! | `DW0480` | **Undeclared story fork** — a flag that gates casts/staging/structure and is set on some playthroughs and not others, belonging to no declared branch point. |
//! | `DW0481` | **Missing `happening`** — a story node that never said what it does to the story (0.8.0+). The forcing function. |
//! | `DW0482` | **Terminality** — a branch that reaches no ending (or not the ending it declares, or not the convergence it declares). |
//! | `DW0483` | **Cast continuity** — the `dw.cast` selector resolves to no cast, or to more than one, at some quest after the fork on some branch. spec-0020 proof 4 extended over the whole post-fork suffix. |
//! | `DW0484` | **Exclusive-content leakage** — content gated on branch A's flags is reachable under branch B's assignment. |
//! | `DW0485` | **Hard event contradiction** — `dies` then acts, `departs` then acts, `seals` then traversed, `loses` then spent, on one branch, with both chronicle lines shown. |
//!
//! Everything here is validation metadata: nothing this module computes reaches
//! the shipped datapack, and the whole module is fenced at `dsl_version 0.8.0`,
//! so a 0.6/0.7 campaign's bytes cannot move.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{
    Campaign, CastEntry, CastPlacement, Diagnostic, EffectSite, Happening, HappeningVerb,
    QuestEffect, for_each_campaign_effect, is_v08,
};

use crate::flow::{Flow, JournalStep, PathStep};

/// A flag forks casts / staging / structure but belongs to no declared branch point.
pub const DW_FORK_UNDECLARED: &str = "DW0480";
/// A story node carries no `happening` declaration (DSL v0.8+).
pub const DW_HAPPENING_MISSING: &str = "DW0481";
/// A declared branch reaches no ending — or not the one it declares.
pub const DW_BRANCH_TERMINAL: &str = "DW0482";
/// A quest's cast selector does not resolve to exactly one placement on a branch.
pub const DW_BRANCH_CAST: &str = "DW0483";
/// Branch-exclusive content is reachable under a sibling branch's assignment.
pub const DW_BRANCH_LEAKAGE: &str = "DW0484";
/// Two chronicle lines on one branch contradict each other.
pub const DW_BRANCH_CONTRADICTION: &str = "DW0485";

// ---------------------------------------------------------------------------
// enumeration
// ---------------------------------------------------------------------------

/// One enumerated branch: a choice of one alternative per declared branch point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumeratedBranch {
    /// The branch's id — one point's branch id, or the `+`-joined tuple when the
    /// campaign declares more than one branch point.
    pub id: String,
    /// Filesystem slug for the chronicle artifact (`branch-chronicle-<slug>.md`).
    pub slug: String,
    /// Which branch was taken at each declared point (point id → branch id).
    pub selection: BTreeMap<String, String>,
    /// Flags pinned SET on this branch.
    pub set: BTreeSet<String>,
    /// Flags pinned UNSET on this branch (its points' `forks_on`, minus `set`).
    pub unset: BTreeSet<String>,
    /// Per selected branch, its `leads_to` (a `quest/…` or an `ending/…`).
    pub leads_to: Vec<String>,
    /// The quests at which the selected points' forks open.
    pub opens_at: Vec<String>,
}

/// Enumerate the campaign's branches — the product of its declared branch
/// points, in declaration order (deterministic by construction).
///
/// A campaign with no declared points enumerates **no** branches: there is
/// nothing to quantify over, and `DW0480` is what proves that claim honest.
pub fn enumerate(c: &Campaign) -> Vec<EnumeratedBranch> {
    let points = &c.quest_plan.content.branch_points;
    if points.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<EnumeratedBranch> = vec![EnumeratedBranch {
        id: String::new(),
        slug: String::new(),
        selection: BTreeMap::new(),
        set: BTreeSet::new(),
        unset: BTreeSet::new(),
        leads_to: Vec::new(),
        opens_at: Vec::new(),
    }];
    for bp in points {
        let mut next = Vec::new();
        for base in &out {
            for b in &bp.branches {
                let mut e = base.clone();
                e.selection
                    .insert(bp.id.as_str().to_string(), b.id.as_str().to_string());
                for f in &b.flags {
                    e.set.insert(f.as_str().to_string());
                }
                for f in &bp.forks_on {
                    if !b.flags.iter().any(|x| x.as_str() == f.as_str()) {
                        e.unset.insert(f.as_str().to_string());
                    }
                }
                e.leads_to.push(b.leads_to.clone());
                e.opens_at.push(bp.opens_at.as_str().to_string());
                next.push(e);
            }
        }
        out = next;
    }
    for e in &mut out {
        let ids: Vec<&str> = e.selection.values().map(|s| s.as_str()).collect();
        e.id = ids.join("+");
        e.slug = ids
            .iter()
            .map(|s| s.trim_start_matches("branch/"))
            .collect::<Vec<_>>()
            .join("+");
    }
    // A flag both set and unset (two points forking on one flag) is pinned SET;
    // `DW0484` then reports the sibling that claims it unset.
    for e in &mut out {
        let set = e.set.clone();
        e.unset.retain(|f| !set.contains(f));
    }
    out
}

// ---------------------------------------------------------------------------
// chronicle
// ---------------------------------------------------------------------------

/// One line of a branch chronicle: a story node's `happening`, at its position in
/// the compiled play order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChronicleLine {
    /// 1-based position in the branch's play order.
    pub n: usize,
    /// What kind of node this is (`quest`, `objective`, `choice`, `effect`,
    /// `ambient`).
    pub kind: &'static str,
    /// The node's id, or its JSON pointer when it has no id of its own.
    pub node: String,
    /// The structured event verb.
    pub verb: HappeningVerb,
    /// What the event happens to, when the node names one.
    pub subject: Option<String>,
    /// The authored line.
    pub text: String,
}

/// A dialogue choice that ENTERS a branch — the player action that forks the story.
///
/// `command` is how it is actuated. A 1.21.11 dialog button is drawn by the CLIENT,
/// so no bot can click one; every option the compiler emits is therefore backed by a
/// `/trigger dw.dlg_<npc> set <n>` the button itself runs, and chatting that line is
/// the player-legal primitive the button stands for (the same substitution the
/// exported critical path has always made for `talk-to` steps — spec-0002, amended
/// 2026-07-30). Carrying it here rather than leaving the harness to derive it keeps
/// the id-mangling (`safe_local`) where it belongs: the harness holds assertions and
/// navigation, never game logic.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntryChoice {
    /// The NPC whose tree the option belongs to.
    pub npc: String,
    /// The option's trigger value, 1-based across that NPC.
    pub option: usize,
    /// The exact chat line that takes the option.
    pub command: String,
}

/// A branch, realized: the world that plays it, its critical path, its chronicle.
#[derive(Clone, Debug)]
pub struct RealizedBranch {
    /// The enumerated branch.
    pub branch: EnumeratedBranch,
    /// The flow world index that realizes it, if one does.
    pub world: Option<usize>,
    /// Its critical path (the flow-level step list, computed under its world).
    pub path: Vec<PathStep>,
    /// The dialogue choices the bot must make to enter it.
    pub entry_choices: Vec<EntryChoice>,
    /// Its chronicle, in compiled play order.
    pub chronicle: Vec<ChronicleLine>,
    /// The endings that fire on it (`campaign-complete` ids; an unnamed
    /// `campaign-complete` contributes the empty string).
    pub endings: Vec<String>,
    /// The quests that complete on it.
    pub completed: BTreeSet<String>,
}

/// Realize every enumerated branch against the flow model.
pub fn realize(c: &Campaign) -> Vec<RealizedBranch> {
    let flow = Flow::new(c);
    enumerate(c)
        .into_iter()
        .map(|b| realize_one(c, &flow, b))
        .collect()
}

fn realize_one(c: &Campaign, flow: &Flow<'_>, branch: EnumeratedBranch) -> RealizedBranch {
    let world = (0..flow.world_count()).find(|&i| {
        let f = flow.world_flags(i);
        branch.set.iter().all(|x| f.contains(x)) && !branch.unset.iter().any(|x| f.contains(x))
    });
    let Some(w) = world else {
        return RealizedBranch {
            branch,
            world: None,
            path: Vec::new(),
            entry_choices: Vec::new(),
            chronicle: Vec::new(),
            endings: Vec::new(),
            completed: BTreeSet::new(),
        };
    };
    let pt = flow.playthrough_in(w);
    let journal = flow.journal(&pt);
    let (chronicle, endings) = chronicle_of(c, &journal);
    let entry_choices = entry_choices(c, &pt, &branch.set);
    RealizedBranch {
        branch,
        world: Some(w),
        path: pt.steps,
        entry_choices,
        chronicle,
        endings,
        completed: flow.world_completed(w),
    }
}

/// The dialogue choices that ENTER this branch: every `talk-to` option on the
/// path that sets one of the branch's pinned-set flags.
///
/// The option index is 1-based **across one NPC's tree**, so it is resolved
/// against the tree of the NPC the step's own `talk-to` objective names — not
/// against every tree in the campaign, where the same ordinal names a different
/// option of a different speaker.
fn entry_choices(
    c: &Campaign,
    pt: &crate::flow::Playthrough,
    set: &BTreeSet<String>,
) -> Vec<EntryChoice> {
    let mut out = Vec::new();
    for step in &pt.steps {
        let Some(n) = step.talk_option else { continue };
        let Some(npc) = talk_to_npc(c, &step.objective) else {
            continue;
        };
        let Some(tree) = c
            .dialogue
            .content
            .dialogues
            .iter()
            .find(|t| t.npc.as_str() == npc)
        else {
            continue;
        };
        let mut k = 0usize;
        for node in &tree.nodes {
            for opt in &node.options {
                k += 1;
                if k != n {
                    continue;
                }
                let sets_branch = opt.effects.iter().any(|e| {
                    matches!(e, delvewright_dsl::DialogueEffect::SetFlag { flag }
                        if set.contains(flag.as_str()))
                });
                if sets_branch {
                    out.push(EntryChoice {
                        npc: npc.to_string(),
                        option: n,
                        command: format!("/trigger {} set {}", crate::plan::dlg_trigger(npc), n),
                    });
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The NPC a `talk-to` objective addresses, if `obj` is one.
fn talk_to_npc<'a>(c: &'a Campaign, obj: &str) -> Option<&'a str> {
    c.quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .find_map(|o| match o {
            delvewright_dsl::Objective::TalkTo { id, npc, .. } if id.as_str() == obj => {
                Some(npc.as_str())
            }
            _ => None,
        })
}

/// Assemble one branch's chronicle from the journal the flow replay produced.
///
/// The SKELETON — which nodes appear, in what order — is derived machine truth:
/// it is exactly the order [`Flow::journal`] replays, which is exactly the order
/// [`Flow::replay`] proves. Only the flesh (each line's `text`) is authored.
fn chronicle_of(c: &Campaign, journal: &[JournalStep]) -> (Vec<ChronicleLine>, Vec<String>) {
    let mut lines: Vec<ChronicleLine> = Vec::new();
    let mut endings: Vec<String> = Vec::new();
    let quest_index: BTreeMap<&str, usize> = c
        .quests
        .content
        .quests
        .iter()
        .enumerate()
        .map(|(i, q)| (q.id.as_str(), i))
        .collect();
    let mut push =
        |kind: &'static str, node: String, h: &Happening, lines: &mut Vec<ChronicleLine>| {
            lines.push(ChronicleLine {
                n: lines.len() + 1,
                kind,
                node,
                verb: h.verb,
                subject: h.subject.clone(),
                text: h.text.clone(),
            });
        };
    let mut announced: BTreeSet<&str> = BTreeSet::new();
    for step in journal {
        let Some(q) = c
            .quests
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == step.quest)
        else {
            continue;
        };
        // A quest's own line lands where the quest first PLAYS, not where its
        // trigger fires: a quest whose trigger fires on a branch that never
        // reaches it has no place in that branch's account.
        if announced.insert(q.id.as_str())
            && let Some(h) = &q.happening
        {
            push("quest", q.id.as_str().to_string(), h, &mut lines);
        }
        if let Some(obj) = q
            .objectives
            .iter()
            .find(|o| o.id().as_str() == step.objective)
            && let Some(h) = obj.happening()
        {
            push("objective", step.objective.clone(), h, &mut lines);
        }
        if let Some(n) = step.talk_option
            && let Some((npc, h)) = option_happening(c, n)
        {
            push("choice", format!("{npc}#{n}"), h, &mut lines);
        }
        let qi = quest_index[step.quest.as_str()];
        if let Some(effs) = q
            .on_objective_complete
            .get(&delvewright_dsl::ObjectiveId(step.objective.clone()))
        {
            let base = format!(
                "/content/quests/{qi}/on_objective_complete/{}",
                step.objective
            );
            for (path, eff) in fired(effs, &base, &step.flags_after) {
                record_effect(&path, eff, &mut lines, &mut endings, &mut push);
            }
        }
        for qid in &step.completed {
            let Some(cq) = c
                .quests
                .content
                .quests
                .iter()
                .find(|q| q.id.as_str() == qid)
            else {
                continue;
            };
            let ci = quest_index[qid.as_str()];
            let base = format!("/content/quests/{ci}/on_complete");
            for (path, eff) in fired(&cq.on_complete, &base, &step.flags_after) {
                record_effect(&path, eff, &mut lines, &mut endings, &mut push);
            }
        }
    }
    // Ambient producers (environment triggers, trap payloads) have no DAG
    // position — `flow` refuses to date them, and so does the chronicle. They are
    // listed after the dated account, and the contradiction proof deliberately
    // does not order them against it.
    for_each_campaign_effect(c, &mut |path, site, eff| {
        if !matches!(site, EffectSite::Trigger { .. } | EffectSite::Trap { .. }) {
            return;
        }
        if let Some(h) = delvewright_dsl_happening(eff) {
            lines.push(ChronicleLine {
                n: lines.len() + 1,
                kind: "ambient",
                node: path.to_string(),
                verb: h.verb,
                subject: h.subject.clone(),
                text: h.text.clone(),
            });
        }
    });
    (lines, endings)
}

fn record_effect(
    path: &str,
    eff: &QuestEffect,
    lines: &mut Vec<ChronicleLine>,
    endings: &mut Vec<String>,
    push: &mut impl FnMut(&'static str, String, &Happening, &mut Vec<ChronicleLine>),
) {
    if let QuestEffect::CampaignComplete { ending, .. } = eff {
        endings.push(
            ending
                .as_ref()
                .map(|e| e.as_str().to_string())
                .unwrap_or_default(),
        );
    }
    if let Some(h) = delvewright_dsl_happening(eff) {
        push("effect", path.to_string(), h, lines);
    }
}

/// The dialogue option `n`'s NPC and `happening`, if it declares one.
fn option_happening(c: &Campaign, n: usize) -> Option<(&str, &Happening)> {
    for tree in &c.dialogue.content.dialogues {
        let mut k = 0usize;
        for node in &tree.nodes {
            for opt in &node.options {
                k += 1;
                if k == n {
                    return opt.happening.as_ref().map(|h| (tree.npc.as_str(), h));
                }
            }
        }
    }
    None
}

/// The effects of `effs` that fire under `flags`, with their JSON pointers —
/// the same gate rule and the same nesting descent [`Flow::fire`] uses, so the
/// chronicle can never claim a beat the replay would have skipped.
fn fired<'a>(
    effs: &'a [QuestEffect],
    base: &str,
    flags: &BTreeSet<String>,
) -> Vec<(String, &'a QuestEffect)> {
    let mut out = Vec::new();
    fired_into(effs, base, flags, &mut out);
    out
}

fn fired_into<'a>(
    effs: &'a [QuestEffect],
    base: &str,
    flags: &BTreeSet<String>,
    out: &mut Vec<(String, &'a QuestEffect)>,
) {
    for (i, e) in effs.iter().enumerate() {
        let gated = !e
            .requires_flags()
            .iter()
            .all(|f| flags.contains(f.as_str()))
            || e.forbids_flags().iter().any(|f| flags.contains(f.as_str()));
        if gated {
            continue;
        }
        let path = format!("{base}/{i}");
        out.push((path.clone(), e));
        match e {
            // Reaction bundles fire at statically unknowable times — the
            // conservative stance `flow` and `continuity` already take.
            QuestEffect::SetCheckpoint { .. }
            | QuestEffect::Bonfire { .. }
            | QuestEffect::BeginStealth { .. } => continue,
            _ => {}
        }
        for (pseg, _k, list) in e.nested_effect_lists_labeled() {
            fired_into(list, &format!("{path}/{pseg}"), flags, out);
        }
    }
}

/// The `happening` of an effect, for the eleven story-node verbs that carry one.
fn delvewright_dsl_happening(eff: &QuestEffect) -> Option<&Happening> {
    match eff {
        QuestEffect::OpenGate { happening, .. }
        | QuestEffect::CloseGate { happening, .. }
        | QuestEffect::CampaignComplete { happening, .. }
        | QuestEffect::SpawnWave { happening, .. }
        | QuestEffect::DespawnNpc { happening, .. }
        | QuestEffect::MoveNpc { happening, .. }
        | QuestEffect::SpawnNpc { happening, .. }
        | QuestEffect::SpawnActor { happening, .. }
        | QuestEffect::DespawnActor { happening, .. }
        | QuestEffect::MoveActor { happening, .. }
        | QuestEffect::UnleashActor { happening, .. } => happening.as_ref(),
        _ => None,
    }
}

/// Is this effect a **story node** — one of the eleven verbs that must declare a
/// `happening` at 0.8.0?
fn is_story_node(eff: &QuestEffect) -> bool {
    matches!(
        eff,
        QuestEffect::OpenGate { .. }
            | QuestEffect::CloseGate { .. }
            | QuestEffect::CampaignComplete { .. }
            | QuestEffect::SpawnWave { .. }
            | QuestEffect::DespawnNpc { .. }
            | QuestEffect::MoveNpc { .. }
            | QuestEffect::SpawnNpc { .. }
            | QuestEffect::SpawnActor { .. }
            | QuestEffect::DespawnActor { .. }
            | QuestEffect::MoveActor { .. }
            | QuestEffect::UnleashActor { .. }
    )
}

// ---------------------------------------------------------------------------
// the proofs
// ---------------------------------------------------------------------------

/// Run every spec-0025 static proof. No-op below `dsl_version 0.8.0`.
pub fn check_branches(c: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    if !is_v08(c.quests.dsl_version.as_str()) && !is_v08(c.quest_plan.dsl_version.as_str()) {
        return d;
    }
    check_happenings(c, &mut d);
    let flow = Flow::new(c);
    check_undeclared_forks(c, &flow, &mut d);
    // The mainline's own skips (`DW0205`) are `crate::analyze`'s to report; here
    // only the ones a BRANCH admits and the campaign's critical path does not, so
    // the same beat is never named twice.
    let main_path = flow.playthrough();
    // Keyed on the objective, not the option: two branches take two different
    // buttons to the same beat, and naming that beat twice tells a reader nothing
    // the mainline row did not.
    let on_main: BTreeSet<String> = if main_path.degenerate {
        BTreeSet::new()
    } else {
        flow.skips(&main_path)
            .into_iter()
            .map(|s| s.objective)
            .collect()
    };
    for b in enumerate(c) {
        let r = realize_one(c, &flow, b);
        check_leakage(c, &flow, &r, &mut d);
        check_terminality(c, &r, &mut d);
        check_cast_continuity(c, &flow, &r, &mut d);
        check_contradictions(&r, &mut d);
        check_branch_skips(c, &flow, &r, &on_main, &mut d);
    }
    d.sort_by(|a, b| (&a.code, &a.path, &a.message).cmp(&(&b.code, &b.path, &b.message)));
    d.dedup_by(|a, b| a.code == b.code && a.path == b.path && a.message == b.message);
    d
}

/// `DW0205`, per branch (task #174). Optionality interacts with branches: a
/// branch's own flag assignment changes which cast scene an NPC wears and which
/// options its gates admit, so a beat that is safely behind a gate on the
/// campaign's critical path can be bare on one branch. This re-runs the
/// participation-minimal walk on the branch's own path and reports only what the
/// mainline walk did not already name.
fn check_branch_skips(
    c: &Campaign,
    flow: &Flow<'_>,
    r: &RealizedBranch,
    on_main: &BTreeSet<String>,
    d: &mut Vec<Diagnostic>,
) {
    let Some(w) = r.world else { return };
    let pt = flow.playthrough_in(w);
    for mut s in flow.skips(&pt) {
        if on_main.contains(&s.objective) {
            continue;
        }
        s.branch = Some(r.branch.id.clone());
        d.push(Diagnostic::error(
            crate::flow::DW_OPTIONAL_GATES_MAINLINE,
            "quests",
            crate::analyze::objective_path(c, &s.objective),
            s.message(),
        ));
    }
}

/// `DW0481` — the forcing function. Every story node states what it does to the
/// story, or the campaign does not compile.
fn check_happenings(c: &Campaign, d: &mut Vec<Diagnostic>) {
    if is_v08(c.quests.dsl_version.as_str()) {
        for (i, q) in c.quests.content.quests.iter().enumerate() {
            if q.happening.is_none() {
                d.push(missing(
                    "quests",
                    format!("/content/quests/{i}/happening"),
                    format!("quest `{}`", q.id.as_str()),
                ));
            }
            for (j, o) in q.objectives.iter().enumerate() {
                if o.happening().is_none() {
                    d.push(missing(
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/happening"),
                        format!("objective `{}`", o.id().as_str()),
                    ));
                }
            }
        }
        let mut sites: Vec<(String, String)> = Vec::new();
        for_each_campaign_effect(c, &mut |path, _site, eff| {
            if is_story_node(eff) && delvewright_dsl_happening(eff).is_none() {
                sites.push((format!("{path}/happening"), eff.verb().to_string()));
            }
        });
        for (path, verb) in sites {
            d.push(missing("quests", path, format!("the `{verb}` beat")));
        }
    }
    if is_v08(c.dialogue.dsl_version.as_str()) {
        for (i, t) in c.dialogue.content.dialogues.iter().enumerate() {
            for (j, n) in t.nodes.iter().enumerate() {
                for (k, o) in n.options.iter().enumerate() {
                    let story_weight = o
                        .effects
                        .iter()
                        .any(|e| matches!(e, delvewright_dsl::DialogueEffect::SetFlag { .. }));
                    if story_weight && o.happening.is_none() {
                        d.push(missing(
                            "dialogue",
                            format!("/content/dialogues/{i}/nodes/{j}/options/{k}/happening"),
                            format!(
                                "the story-weight option `{}` of `{}`",
                                o.label,
                                t.npc.as_str()
                            ),
                        ));
                    }
                }
            }
        }
    }
}

fn missing(stage: &str, path: String, what: String) -> Diagnostic {
    Diagnostic::error(
        DW_HAPPENING_MISSING,
        stage,
        path,
        format!(
            "{what} declares no `happening` — say what this node does to the story, as one of the \
             structured verbs (`dies`, `survives`, `departs`, `arrives`, `learns`, `believes`, \
             `gains`, `loses`, `opens`, `seals`) plus one line of prose and, where the beat is \
             about somebody, a `subject`. This is the forcing function: a design that never got \
             written down node by node cannot compile, and the per-branch chronicle the narrative \
             review reads is assembled from exactly these lines. Do NOT paper over it with a \
             placeholder line — an unread chronicle is worse than none"
        ),
    )
}

/// `DW0480` — a flag that forks the story but belongs to no declared point.
///
/// "Forks" is decided, not guessed: the flag must be **read** by a cast
/// placement, a story-node effect (or a bundle containing one), an objective or
/// an environment trigger that stages one, AND it must be set in some enumerated
/// world and not in another. A flag every playthrough sets is ordinary
/// sequencing and is never reported.
fn check_undeclared_forks(c: &Campaign, flow: &Flow<'_>, d: &mut Vec<Diagnostic>) {
    if !is_v08(c.quest_plan.dsl_version.as_str()) {
        return;
    }
    let declared: BTreeSet<String> = c
        .quest_plan
        .content
        .branch_points
        .iter()
        .flat_map(|bp| bp.forks_on.iter().map(|f| f.as_str().to_string()))
        .collect();
    let mut readers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let note = |flag: &str, what: String, m: &mut BTreeMap<String, BTreeSet<String>>| {
        m.entry(flag.to_string()).or_default().insert(what);
    };
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            for f in o.requires_flags().iter().chain(o.forbids_flags()) {
                note(
                    f.as_str(),
                    format!("objective `{}`", o.id().as_str()),
                    &mut readers,
                );
            }
        }
        for (npc, entry) in &q.cast {
            for p in entry.placements() {
                for f in p.requires_flags.iter().chain(p.forbids_flags.iter()) {
                    note(
                        f.as_str(),
                        format!("the cast of `{}` in `{}`", npc.as_str(), q.id.as_str()),
                        &mut readers,
                    );
                }
            }
        }
    }
    for_each_campaign_effect(c, &mut |path, _site, eff| {
        if !stages_a_story_node(eff) {
            return;
        }
        for f in eff.requires_flags().iter().chain(eff.forbids_flags()) {
            note(f.as_str(), format!("the staging at `{path}`"), &mut readers);
        }
    });
    for t in &c.quests.content.triggers {
        if !t.effects.iter().any(stages_a_story_node) {
            continue;
        }
        for f in t.requires_flags.iter().chain(t.forbids_flags.iter()) {
            note(
                f.as_str(),
                format!("trigger `{}`", t.id.as_str()),
                &mut readers,
            );
        }
    }

    let worlds: Vec<BTreeSet<String>> = (0..flow.world_count())
        .map(|i| flow.world_flags(i))
        .collect();
    for (flag, sites) in readers {
        if declared.contains(&flag) {
            continue;
        }
        let some = worlds.iter().any(|w| w.contains(&flag));
        let none = worlds.iter().any(|w| !w.contains(&flag));
        if !(some && none) {
            continue;
        }
        let where_ = sites.iter().cloned().collect::<Vec<_>>().join(", ");
        d.push(Diagnostic::error(
            DW_FORK_UNDECLARED,
            "quest-plan",
            "/content/branch_points".to_string(),
            format!(
                "`{flag}` is an UNDECLARED story fork: some playthroughs set it and others do not, \
                 and it gates {where_} — so it decides who is where and what the world looks \
                 like, on a split no `branch_points` entry owns. An undeclared fork is a branch \
                 nothing verifies, which is exactly how a campaign ships with the cast ledger on \
                 one branch and the bodies on the other. Prescription: declare the branch point \
                 (its `forks_on`, the quest it `opens_at`, and each branch with what it `leads_to`) \
                 so the compiler can enumerate and prove it. Do NOT silence this by ungating the \
                 content — the gate is the story"
            ),
        ));
    }
}

/// Does this effect stage a story node — itself, or anywhere inside it?
fn stages_a_story_node(eff: &QuestEffect) -> bool {
    if is_story_node(eff) {
        return true;
    }
    eff.nested_effect_lists()
        .iter()
        .any(|l| l.iter().any(stages_a_story_node))
}

/// `DW0484` — a branch's flag assignment is not exclusive: every playthrough that
/// realizes its set flags also produces a flag it pins unset.
fn check_leakage(c: &Campaign, flow: &Flow<'_>, r: &RealizedBranch, d: &mut Vec<Diagnostic>) {
    if r.world.is_some() {
        return;
    }
    let candidates: Vec<usize> = (0..flow.world_count())
        .filter(|&i| {
            let f = flow.world_flags(i);
            r.branch.set.iter().all(|x| f.contains(x))
        })
        .collect();
    if candidates.is_empty() {
        // Not reachable at all — `DW0482` owns that.
        return;
    }
    let mut leaked: BTreeSet<String> = BTreeSet::new();
    for i in candidates {
        let f = flow.world_flags(i);
        for x in &r.branch.unset {
            if f.contains(x) {
                leaked.insert(x.clone());
            }
        }
    }
    let producers = flag_producers(c, &leaked);
    d.push(Diagnostic::error(
        DW_BRANCH_LEAKAGE,
        "quest-plan",
        "/content/branch_points".to_string(),
        format!(
            "branch `{}` LEAKS its siblings' content: its assignment pins {} unset, but every \
             playthrough that takes this branch produces {} anyway ({}). Content gated on a \
             sibling's flag is therefore reachable HERE — a mourning scene on the branch where \
             nobody died, which is a build error and not a review note. Branch assignment: {}. \
             Prescription: make the producer exclusive to the branch that owns it (gate it on that \
             branch's flag, or move it onto that branch's quest); do NOT relax the branch \
             declaration to admit the leak",
            r.branch.id,
            join(&r.branch.unset),
            join(&leaked),
            if producers.is_empty() {
                "no `set-flag` found — the flag is ambient (an environment trigger or a trap \
                 disarm), which fires on every branch by construction"
                    .to_string()
            } else {
                format!("set by {}", producers.join(", "))
            },
            assignment(&r.branch),
        ),
    ));
}

/// Where a flag is produced, for the leakage message.
fn flag_producers(c: &Campaign, flags: &BTreeSet<String>) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for_each_campaign_effect(c, &mut |path, _site, eff| {
        if let QuestEffect::SetFlag { flag, .. } = eff
            && flags.contains(flag.as_str())
        {
            out.insert(format!("`{path}`"));
        }
    });
    for t in &c.dialogue.content.dialogues {
        for n in &t.nodes {
            for o in &n.options {
                for e in &o.effects {
                    if let delvewright_dsl::DialogueEffect::SetFlag { flag } = e
                        && flags.contains(flag.as_str())
                    {
                        out.insert(format!("the `{}` option of `{}`", o.label, t.npc.as_str()));
                    }
                }
            }
        }
    }
    out.into_iter().collect()
}

/// `DW0482` — every branch reaches an ending, and the one it declares.
fn check_terminality(c: &Campaign, r: &RealizedBranch, d: &mut Vec<Diagnostic>) {
    let Some(bp) = point_of(c, &r.branch) else {
        return;
    };
    if r.world.is_none() {
        d.push(Diagnostic::error(
            DW_BRANCH_TERMINAL,
            "quest-plan",
            "/content/branch_points".to_string(),
            format!(
                "branch `{}` is NOT REACHABLE, so it reaches no ending: no playthrough of this \
                 campaign sets {} while leaving {} unset. A declared branch nobody can take is a \
                 branch nothing proves. Branch assignment: {}. Prescription: give the fork a \
                 dialogue option (or a beat) that really sets this branch's flags, or drop the \
                 branch from `{}`",
                r.branch.id,
                join(&r.branch.set),
                join(&r.branch.unset),
                assignment(&r.branch),
                bp,
            ),
        ));
        return;
    }
    for (i, leads_to) in r.branch.leads_to.iter().enumerate() {
        if let Some(rest) = leads_to.strip_prefix("ending/") {
            let want = format!("ending/{rest}");
            if !r.endings.contains(&want) {
                d.push(Diagnostic::error(
                    DW_BRANCH_TERMINAL,
                    "quest-plan",
                    "/content/branch_points".to_string(),
                    format!(
                        "branch `{}` declares it runs to `{want}`, but on its own playthrough the \
                         `campaign-complete` that fires is {}. Branch assignment: {}. \
                         Prescription: put the `campaign-complete` carrying `ending: \"{want}\"` on \
                         a beat this branch actually reaches, or point the branch at the ending it \
                         really has",
                        r.branch.id,
                        if r.endings.is_empty() {
                            "NONE — the branch never ends the delve".to_string()
                        } else {
                            join(&r.endings.iter().cloned().collect())
                        },
                        assignment(&r.branch),
                    ),
                ));
            }
        } else if let Some(q) = leads_to.strip_prefix("quest/") {
            let q = format!("quest/{q}");
            if !r.completed.contains(&q) {
                d.push(Diagnostic::error(
                    DW_BRANCH_TERMINAL,
                    "quest-plan",
                    format!("/content/branch_points/{i}"),
                    format!(
                        "branch `{}` declares it converges at `{q}`, but that quest does not \
                         complete on its playthrough — so the branch runs off the end of the \
                         story. Branch assignment: {}. Prescription: make `{q}` reachable under \
                         this branch's flags, or declare the ending this branch really runs to",
                        r.branch.id,
                        assignment(&r.branch),
                    ),
                ));
            } else if r.endings.is_empty() {
                d.push(Diagnostic::error(
                    DW_BRANCH_TERMINAL,
                    "quest-plan",
                    format!("/content/branch_points/{i}"),
                    format!(
                        "branch `{}` converges at `{q}` but no `campaign-complete` fires anywhere \
                         on its playthrough — converging is not ending. Branch assignment: {}",
                        r.branch.id,
                        assignment(&r.branch),
                    ),
                ));
            }
        }
    }
}

/// `DW0483` — spec-0020's proof 4, extended over the whole post-fork suffix.
///
/// Later-declaration-wins makes the suffix load-bearing: it is not enough that a
/// branch-divergent NPC *has* per-branch casts at the fork, every later quest's
/// selector must still resolve to exactly one placement — this branch's — under
/// this branch's pinned flags. Round 13 broke precisely there.
fn check_cast_continuity(
    c: &Campaign,
    flow: &Flow<'_>,
    r: &RealizedBranch,
    d: &mut Vec<Diagnostic>,
) {
    let Some(w) = r.world else { return };
    let pt = flow.playthrough_in(w);
    let journal = flow.journal(&pt);
    // Flag state when each quest's first objective is attempted.
    let mut at_open: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for step in &journal {
        at_open
            .entry(step.quest.as_str())
            .or_insert_with(|| step.flags_before.clone());
    }
    let suffix = post_fork_quests(&pt.quests, &r.branch.opens_at);
    for qid in &suffix {
        let Some(q) = c
            .quests
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == qid)
        else {
            continue;
        };
        let Some(flags) = at_open.get(qid.as_str()) else {
            continue;
        };
        for (npc, entry) in &q.cast {
            let CastEntry::Branches(list) = entry else {
                continue;
            };
            let hits: Vec<usize> = list
                .iter()
                .enumerate()
                .filter(|(_, p)| crate::cast::selects(p, flags))
                .map(|(i, _)| i)
                .collect();
            if hits.len() == 1 {
                continue;
            }
            let detail = if hits.is_empty() {
                "NO per-branch placement selects".to_string()
            } else {
                format!(
                    "{} placements select at once ({}); emission dispatches the LAST clause, so \
                     this branch shows `{}`",
                    hits.len(),
                    hits.iter()
                        .map(|&i| describe(&list[i]))
                        .collect::<Vec<_>>()
                        .join(" and "),
                    describe(&list[*hits.last().unwrap()]),
                )
            };
            d.push(Diagnostic::error(
                DW_BRANCH_CAST,
                "quests",
                format!("/content/quests/{qid}/cast/{}", npc.as_str()),
                format!(
                    "cast continuity breaks on branch `{}` at `{qid}`: for `{}`, {detail}. The \
                     `dw.cast` selector must resolve to exactly ONE placement per branch at EVERY \
                     quest after the fork — later declarations win, so a placement left ungated \
                     (or gated on the other branch's flag) keeps governing long past the beat that \
                     wrote it. That is the island round-13 defect: the fork moved the ledger and \
                     never moved the bodies. Branch assignment: {}. Prescription: gate each \
                     placement on the flags of the branch it belongs to — every branch, every \
                     post-fork quest. Do NOT leave one ungated as a fallback: a fallback selects \
                     on the branch that already has its own",
                    r.branch.id,
                    npc.as_str(),
                    assignment(&r.branch),
                ),
            ));
        }
    }
}

/// Quests **strictly after** the earliest declared fork, in playthrough order.
///
/// The fork quest itself is excluded on purpose: a branch is not decided until
/// its quest resolves, so during `opens_at` the flag state is by construction
/// pre-fork and a per-branch cast there could never select. The suffix is
/// exactly where round 13 broke — the bodies that stayed on the other branch
/// were all in *later* quests.
fn post_fork_quests(order: &[String], opens_at: &[String]) -> Vec<String> {
    let start = opens_at
        .iter()
        .filter_map(|q| order.iter().position(|x| x == q))
        .min()
        .map(|i| i + 1)
        .unwrap_or(0);
    if start >= order.len() {
        return Vec::new();
    }
    order[start..].to_vec()
}

fn describe(p: &CastPlacement) -> String {
    let gates = if p.requires_flags.is_empty() && p.forbids_flags.is_empty() {
        "ungated".to_string()
    } else {
        let mut g: Vec<String> = p
            .requires_flags
            .iter()
            .map(|f| f.as_str().to_string())
            .collect();
        g.extend(p.forbids_flags.iter().map(|f| format!("!{}", f.as_str())));
        g.join(" & ")
    };
    format!("`{}` [{gates}]", p.at.token())
}

/// `DW0485` — hard event contradictions, per branch, over the chronicle order.
///
/// Four rules, each decidable from the structured verbs alone:
/// 1. `dies(S)` then any later ACT by `S` — a dead man does nothing.
/// 2. `departs(S)` then a later beat by `S` with no `arrives(S)` between.
/// 3. `seals(S)` then a later beat about `S` that is not `opens(S)`.
/// 4. `loses(S)` then a later `loses(S)` with no `gains(S)` between.
///
/// Ambient lines (triggers, traps) are excluded: `flow` refuses to date them, so
/// ordering them against the dated account would invent a sequence.
fn check_contradictions(r: &RealizedBranch, d: &mut Vec<Diagnostic>) {
    let dated: Vec<&ChronicleLine> = r.chronicle.iter().filter(|l| l.kind != "ambient").collect();
    #[derive(Clone)]
    struct State<'a> {
        line: &'a ChronicleLine,
        verb: HappeningVerb,
    }
    let mut state: BTreeMap<&str, State<'_>> = BTreeMap::new();
    for l in &dated {
        let Some(subject) = l.subject.as_deref() else {
            continue;
        };
        // `learns`/`believes` are EPISTEMIC: their subject is what the beat is
        // *about*, not somebody acting, and a living character may perfectly
        // well believe something about a dead one. They are therefore never
        // contradictions — "Elpenor mourns a man standing beside him" is exactly
        // the class spec-0025 leaves to the chronicle's human reader, because no
        // verb makes it decidable.
        let acts = !matches!(l.verb, HappeningVerb::Learns | HappeningVerb::Believes);
        if acts && let Some(prev) = state.get(subject) {
            let bad = match prev.verb {
                HappeningVerb::Dies => Some("acts after it dies"),
                HappeningVerb::Departs if l.verb != HappeningVerb::Arrives => {
                    Some("acts while it is offstage")
                }
                HappeningVerb::Seals if l.verb != HappeningVerb::Opens => {
                    Some("is used after it is sealed")
                }
                HappeningVerb::Loses if l.verb == HappeningVerb::Loses => {
                    Some("is spent twice over")
                }
                _ => None,
            };
            if let Some(why) = bad {
                d.push(Diagnostic::error(
                    DW_BRANCH_CONTRADICTION,
                    "quests",
                    format!("/content/quests#branch/{}", r.branch.id),
                    format!(
                        "on branch `{}`, `{subject}` {why}:\n    #{} [{}] {} — {}\n    #{} [{}] {} \
                         — {}\nBranch assignment: {}. The two lines cannot both be true of one \
                         playthrough. Prescription: fix whichever beat is on the wrong branch — \
                         usually the later one belongs to the sibling branch and needs its flag \
                         gate. Do NOT reword the `happening` to hide the clash: the verbs are the \
                         only part of the chronicle a machine can check",
                        r.branch.id,
                        prev.line.n,
                        verb_name(prev.verb),
                        prev.line.node,
                        prev.line.text,
                        l.n,
                        verb_name(l.verb),
                        l.node,
                        l.text,
                        assignment(&r.branch),
                    ),
                ));
            }
        }
        if !acts {
            continue;
        }
        let carry = matches!(
            l.verb,
            HappeningVerb::Dies
                | HappeningVerb::Departs
                | HappeningVerb::Seals
                | HappeningVerb::Loses
        );
        if carry {
            state.insert(
                subject,
                State {
                    line: l,
                    verb: l.verb,
                },
            );
        } else {
            state.remove(subject);
        }
    }
}

/// The name of the branch point a branch belongs to (single-point campaigns
/// report the point; a product tuple reports them all).
fn point_of(c: &Campaign, b: &EnumeratedBranch) -> Option<String> {
    let names: Vec<String> = c
        .quest_plan
        .content
        .branch_points
        .iter()
        .filter(|bp| b.selection.contains_key(bp.id.as_str()))
        .map(|bp| bp.id.as_str().to_string())
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// The branch's flag assignment, rendered for a diagnostic.
fn assignment(b: &EnumeratedBranch) -> String {
    let mut parts: Vec<String> = b.set.iter().map(|f| format!("{f}=set")).collect();
    parts.extend(b.unset.iter().map(|f| format!("{f}=unset")));
    if parts.is_empty() {
        "(no flags)".to_string()
    } else {
        parts.join(", ")
    }
}

fn join(s: &BTreeSet<String>) -> String {
    if s.is_empty() {
        "nothing".to_string()
    } else {
        s.iter()
            .map(|x| format!("`{x}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The kebab-case spelling of a verb — the one the DSL uses.
pub fn verb_name(v: HappeningVerb) -> &'static str {
    match v {
        HappeningVerb::Dies => "dies",
        HappeningVerb::Survives => "survives",
        HappeningVerb::Departs => "departs",
        HappeningVerb::Arrives => "arrives",
        HappeningVerb::Learns => "learns",
        HappeningVerb::Believes => "believes",
        HappeningVerb::Gains => "gains",
        HappeningVerb::Loses => "loses",
        HappeningVerb::Opens => "opens",
        HappeningVerb::Seals => "seals",
    }
}

// ---------------------------------------------------------------------------
// artifacts
// ---------------------------------------------------------------------------

/// The spec-0025 validation artifacts: `validation/branch-plan.json` and one
/// `validation/branch-chronicle-<branch>.md` per enumerated branch.
///
/// Validation metadata only — never part of the shipped datapack, listed in the
/// manifest exactly like `critical-path-waypoints.json`. Empty for a campaign
/// that declares no branch points, so nothing changes for anybody who has not
/// opted in.
pub fn artifacts(c: &Campaign) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let branches = realize(c);
    if branches.is_empty() {
        return out;
    }
    let plan = serde_json::json!({
        "version": c.quest_plan.dsl_version,
        "campaign_id": c.quest_plan.campaign_id.as_str(),
        "branches": branches.iter().map(|r| serde_json::json!({
            "id": r.branch.id,
            "chronicle": format!("branch-chronicle-{}.md", r.branch.slug),
            // The EXECUTABLE path the harness walks for this branch, in the
            // `critical-path.json` contract (emitted by `emit::branch_paths`).
            // `null` for an unreachable branch: there is no world that plays it,
            // so the harness reports it skipped — named, never silently absent.
            "path": if r.world.is_some() {
                serde_json::Value::String(format!("branch-path-{}.json", r.branch.slug))
            } else {
                serde_json::Value::Null
            },
            "selection": r.branch.selection,
            "flags": {
                "set": r.branch.set.iter().collect::<Vec<_>>(),
                "unset": r.branch.unset.iter().collect::<Vec<_>>(),
            },
            "opens_at": r.branch.opens_at,
            "leads_to": r.branch.leads_to,
            "reachable": r.world.is_some(),
            "entry_choices": r.entry_choices.iter().map(|e| serde_json::json!({
                "npc": e.npc,
                "option": e.option,
                // The chat line the option's dialog button runs. A dialog button is
                // client-rendered and unclickable by a bot, so this is the
                // player-legal actuation the harness sends — and what it asserts the
                // branch path really contains, so a "branch run" that never made the
                // branching choice cannot pass as one.
                "command": e.command,
            })).collect::<Vec<_>>(),
            "endings": r.endings,
            "critical_path": r.path.iter().map(|s| {
                let mut o = serde_json::Map::new();
                o.insert("quest".into(), s.quest.clone().into());
                o.insert("objective".into(), s.objective.clone().into());
                if let Some(n) = s.talk_option {
                    o.insert("talk_option".into(), n.into());
                }
                serde_json::Value::Object(o)
            }).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    let mut bytes = serde_json::to_vec_pretty(&plan).expect("branch plan serializes");
    bytes.push(b'\n');
    out.insert("validation/branch-plan.json".to_string(), bytes);
    for r in &branches {
        out.insert(
            format!("validation/branch-chronicle-{}.md", r.branch.slug),
            chronicle_markdown(c, r).into_bytes(),
        );
    }
    out
}

/// The chronicle (流水账): one branch's storyline in compiled play order, from
/// first beat to ending, readable end to end against the campaign's DESIGN.md.
fn chronicle_markdown(c: &Campaign, r: &RealizedBranch) -> String {
    let mut s = String::new();
    s.push_str(&format!("# Branch chronicle — `{}`\n\n", r.branch.id));
    s.push_str(&format!(
        "Campaign: `{}`\n\n",
        c.quest_plan.campaign_id.as_str()
    ));
    s.push_str(&format!("Flag assignment: {}\n\n", assignment(&r.branch)));
    if r.world.is_none() {
        s.push_str(
            "**This branch is not reachable** — no playthrough realizes its flag assignment, so \
             there is no storyline to account for. See `DW0482`.\n",
        );
        return s;
    }
    if r.entry_choices.is_empty() {
        s.push_str("Entered by: no dialogue choice (the branch's flags come from elsewhere)\n\n");
    } else {
        s.push_str("Entered by: ");
        s.push_str(
            &r.entry_choices
                .iter()
                .map(|e| format!("option #{} of `{}`", e.option, e.npc))
                .collect::<Vec<_>>()
                .join(", "),
        );
        s.push_str("\n\n");
    }
    s.push_str("## The storyline\n\n");
    let dated: Vec<&ChronicleLine> = r.chronicle.iter().filter(|l| l.kind != "ambient").collect();
    if dated.is_empty() {
        s.push_str("_(no node on this branch declares a happening)_\n");
    }
    for l in &dated {
        s.push_str(&format!(
            "{}. **{}** ({} `{}`) — {}\n",
            l.n,
            verb_name(l.verb),
            l.kind,
            l.node,
            l.text
        ));
        if let Some(sub) = &l.subject {
            s.push_str(&format!("   - subject: `{sub}`\n"));
        }
    }
    let ambient: Vec<&ChronicleLine> = r.chronicle.iter().filter(|l| l.kind == "ambient").collect();
    if !ambient.is_empty() {
        s.push_str(
            "\n## Ambient beats (no fixed position)\n\nThese fire from environment triggers or \
             trap payloads, which have no place in the quest DAG — the compiler refuses to date \
             them, and so does this account.\n\n",
        );
        for l in &ambient {
            s.push_str(&format!(
                "- **{}** (`{}`) — {}\n",
                verb_name(l.verb),
                l.node,
                l.text
            ));
        }
    }
    s.push_str("\n## Endings reached\n\n");
    if r.endings.is_empty() {
        s.push_str("_(none — this branch never fires `campaign-complete`)_\n");
    } else {
        for e in &r.endings {
            s.push_str(&format!(
                "- `{}`\n",
                if e.is_empty() { "(unnamed)" } else { e }
            ));
        }
    }
    s
}
