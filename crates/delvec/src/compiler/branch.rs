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
//! An assignment is then realized against [`crate::compiler::flow`]'s enumerated worlds: a
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
//! | `DW0485` | **Hard event contradiction** — `dies` then acts, `departs` then acts, `seals` then traversed, `loses` then spent, on one branch, in any play order the branch admits, with both chronicle lines shown. |
//! | `DW0927` | **Unproven contradiction question** — the every-order search behind `DW0485` reached its bound on a branch, so the branch is refused rather than called clean. |
//!
//! Everything here is validation metadata: nothing this module computes reaches
//! the shipped datapack.

use delvewright_dsl::Verb;
use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{
    Campaign, CastEntry, CastPlacement, Diagnostic, EffectSite, Happening, HappeningVerb,
    QuestEffect, for_each_campaign_effect,
};

use crate::compiler::flow::{Flow, JournalStep, PathStep};
use delvewright_dsl::{DwCode, ExitTier};

// spec-0025's six proofs are **obligations**: each one requires the campaign
// to HAVE something — a declared branch point, a `happening`, a per-branch
// cast placement.

/// A flag forks casts / staging / structure but belongs to no declared branch point.
pub const DW_FORK_UNDECLARED: DwCode = DwCode::new("DW0480", ExitTier::Build);
/// A story node carries no `happening` declaration (DSL v0.8+).
pub const DW_HAPPENING_MISSING: DwCode = DwCode::new("DW0481", ExitTier::Build);
/// A declared branch reaches no ending — or not the one it declares.
pub const DW_BRANCH_TERMINAL: DwCode = DwCode::new("DW0482", ExitTier::Build);
/// A quest's cast selector does not resolve to exactly one placement on a branch.
pub const DW_BRANCH_CAST: DwCode = DwCode::new("DW0483", ExitTier::Build);
/// Branch-exclusive content is reachable under a sibling branch's assignment.
pub const DW_BRANCH_LEAKAGE: DwCode = DwCode::new("DW0484", ExitTier::Build);
/// Two chronicle lines on one branch contradict each other.
pub const DW_BRANCH_CONTRADICTION: DwCode = DwCode::new("DW0485", ExitTier::Build);
/// Whether any play order of a branch shows a `DW0485` clash is unproven: the
/// every-order search reached [`MAX_ORDER_STATES`].
pub const DW_BRANCH_CONTRADICTION_UNPROVEN: DwCode = DwCode::new("DW0927", ExitTier::Build);

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
    pt: &crate::compiler::flow::Playthrough,
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
                        command: format!(
                            "/trigger {} set {}",
                            crate::compiler::plan::dlg_trigger(npc),
                            n
                        ),
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
    let mut w = Chronicler::new(c);
    for step in journal {
        w.push_step(step);
    }
    w.finish()
}

/// The dated chronicle, written one journal step at a time — so a walk that
/// chooses its own order ([`check_every_order`]) writes its account by the same
/// rule the exported order's chronicle is written by, and can look at the lines
/// one more step would add before it takes that step.
#[derive(Clone)]
struct Chronicler<'c> {
    c: &'c Campaign,
    quest_index: std::rc::Rc<BTreeMap<&'c str, usize>>,
    /// The lines written and not yet taken ([`Chronicler::take_lines`]).
    lines: Vec<ChronicleLine>,
    /// How many lines have been written in all — the next line's number.
    written: usize,
    endings: Vec<String>,
    /// The quests whose own line has been written.
    announced: BTreeSet<&'c str>,
}

impl<'c> Chronicler<'c> {
    fn new(c: &'c Campaign) -> Self {
        Chronicler {
            c,
            quest_index: std::rc::Rc::new(
                c.quests
                    .content
                    .quests
                    .iter()
                    .enumerate()
                    .map(|(i, q)| (q.id.as_str(), i))
                    .collect(),
            ),
            lines: Vec::new(),
            written: 0,
            endings: Vec::new(),
            announced: BTreeSet::new(),
        }
    }

    /// A node's own line. `subject` is passed in rather than read off the
    /// `happening`, because an effect's subject is the one **derivation**
    /// (`QuestEffect::happening_subject`, spec-0071 §3) and a chronicle that
    /// re-read `h.subject` here would be a second answer to a question the DSL
    /// already answers — the beat would be about one thing for the proof and
    /// another for the reader.
    fn push(&mut self, kind: &'static str, node: String, h: &Happening, subject: Option<String>) {
        self.written += 1;
        self.lines.push(ChronicleLine {
            n: self.written,
            kind,
            node,
            verb: h.verb,
            subject,
            text: h.text.clone(),
        });
    }

    fn push_step(&mut self, step: &JournalStep) {
        let c = self.c;
        let Some(q) = c
            .quests
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == step.quest)
        else {
            return;
        };
        // A quest's own line lands where the quest first PLAYS, not where its
        // trigger fires: a quest whose trigger fires on a branch that never
        // reaches it has no place in that branch's account.
        if self.announced.insert(q.id.as_str())
            && let Some(h) = &q.happening
        {
            self.push("quest", q.id.as_str().to_string(), h, h.subject.clone());
        }
        if let Some(obj) = q
            .objectives
            .iter()
            .find(|o| o.id().as_str() == step.objective)
            && let Some(h) = obj.happening()
        {
            self.push("objective", step.objective.clone(), h, h.subject.clone());
        }
        // The dialogue option this branch takes to complete a `talk-to` beat —
        // the place a fork's divergence actually lives, since a dialogue effect
        // carries no `happening` of its own and the option is the only thing on
        // that side of the campaign that does. Named by the node it stands in
        // and its ordinal, so a citation table can point at it.
        if let Some(n) = step.talk_option
            && let Some(npc) = talk_to_npc(c, &step.objective)
            && let Some((node, opt)) = option_at(c, npc, n)
            && let Some(h) = &opt.happening
        {
            self.push("choice", format!("{npc} {node}#{n}"), h, h.subject.clone());
        }
        let qi = self.quest_index[step.quest.as_str()];
        if let Some(effs) = q
            .on_objective_complete
            .get(&delvewright_dsl::ObjectiveId(step.objective.clone()))
        {
            let base = format!(
                "/content/quests/{qi}/on_objective_complete/{}",
                step.objective
            );
            for (path, eff) in fired(effs, &base, &step.fired) {
                self.record_effect(&path, eff);
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
            let ci = self.quest_index[qid.as_str()];
            let base = format!("/content/quests/{ci}/on_complete");
            for (path, eff) in fired(&cq.on_complete, &base, &step.fired) {
                self.record_effect(&path, eff);
            }
        }
    }

    fn record_effect(&mut self, path: &str, eff: &QuestEffect) {
        if let Verb::CampaignComplete { ending, .. } = &eff.verb {
            self.endings.push(
                ending
                    .as_ref()
                    .map(|e| e.as_str().to_string())
                    .unwrap_or_default(),
            );
        }
        if let Some(h) = eff.happening.as_ref() {
            // The subject an effect's beat is about: stated, or derived from the
            // effect's own single object (spec-0071 §3).
            let subject = eff.happening_subject().map(|s| s.id.to_string());
            self.push("effect", path.to_string(), h, subject);
        }
    }

    /// The lines written since the last call, handed over: a walk that only
    /// reads each step's new lines keeps none, so cloning it is cheap.
    fn take_lines(&mut self) -> Vec<ChronicleLine> {
        std::mem::take(&mut self.lines)
    }

    /// The dated account, with the ambient lines listed after it.
    fn finish(mut self) -> (Vec<ChronicleLine>, Vec<String>) {
        // Ambient producers (environment triggers, trap payloads) have no DAG
        // position — `flow` refuses to date them, and so does the chronicle. They
        // are listed after the dated account, and the contradiction proof
        // deliberately does not order them against it.
        let lines = &mut self.lines;
        let written = &mut self.written;
        for_each_campaign_effect(self.c, &mut |path, site, eff| {
            if !matches!(site, EffectSite::Trigger { .. } | EffectSite::Trap { .. }) {
                return;
            }
            if let Some(h) = eff.happening.as_ref() {
                *written += 1;
                lines.push(ChronicleLine {
                    n: *written,
                    kind: "ambient",
                    node: path.to_string(),
                    verb: h.verb,
                    subject: eff.happening_subject().map(|s| s.id.to_string()),
                    text: h.text.clone(),
                });
            }
        });
        (self.lines, self.endings)
    }
}

/// Option `n` of **`npc`'s own tree**: the node it stands in, and its
/// `happening` if it declares one.
///
/// The ordinal is 1-based across one NPC's tree — that is the scope
/// `flow::flatten_trees` assigns it in and the scope `plan::plan_npc` emits it
/// in — so it is resolved against that NPC's tree and no other. Resolving it
/// against a campaign-wide enumeration returns an honest answer about a
/// different speaker's option: the same defect [`entry_choices`] avoids by
/// taking the tree first.
fn option_at<'a>(
    c: &'a Campaign,
    npc: &str,
    n: usize,
) -> Option<(&'a str, &'a delvewright_dsl::DialogueOption)> {
    let tree = c
        .dialogue
        .content
        .dialogues
        .iter()
        .find(|t| t.npc.as_str() == npc)?;
    let mut k = 0usize;
    for node in &tree.nodes {
        for opt in &node.options {
            k += 1;
            if k == n {
                return Some((node.id.as_str(), opt));
            }
        }
    }
    None
}

/// The effects of `effs` (rooted at `base`) the replay fired at this step,
/// with their JSON pointers, in firing order. Whether a beat played is the
/// replay's answer ([`JournalStep::fired`], recorded by [`Flow`]'s one gate
/// test where the replay reached the effect), never a second reading of the
/// gate here — so the chronicle can never claim a beat the replay skipped, nor
/// skip one it played.
fn fired<'a>(
    effs: &'a [QuestEffect],
    base: &str,
    played: &BTreeSet<String>,
) -> Vec<(String, &'a QuestEffect)> {
    fn walk<'a>(
        effs: &'a [QuestEffect],
        base: &str,
        played: &BTreeSet<String>,
        out: &mut Vec<(String, &'a QuestEffect)>,
    ) {
        for (i, e) in effs.iter().enumerate() {
            let path = format!("{base}/{i}");
            if !played.contains(&path) {
                continue;
            }
            out.push((path.clone(), e));
            for (pseg, _k, list) in e.nested_effect_lists_labeled() {
                walk(list, &format!("{path}/{pseg}"), played, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(effs, base, played, &mut out);
    out
}

/// Is this effect a **story node** — one of the eleven verbs that must declare a
/// `happening` at 0.8.0?
fn is_story_node(eff: &QuestEffect) -> bool {
    matches!(
        &eff.verb,
        Verb::OpenGate { .. }
            | Verb::CloseGate { .. }
            | Verb::CampaignComplete { .. }
            | Verb::SpawnWave { .. }
            | Verb::DespawnNpc { .. }
            | Verb::MoveNpc { .. }
            | Verb::SpawnNpc { .. }
            | Verb::SpawnActor { .. }
            | Verb::DespawnActor { .. }
            | Verb::MoveActor { .. }
            | Verb::UnleashActor { .. }
    )
}

// ---------------------------------------------------------------------------
// the proofs
// ---------------------------------------------------------------------------

/// Run every spec-0025 static proof.
pub fn check_branches(c: &Campaign) -> Vec<Diagnostic> {
    check_branches_bound(c).0
}

/// [`check_branches`], with what `DW0485`'s every-order search examined.
pub fn check_branches_bound(c: &Campaign) -> (Vec<Diagnostic>, ContradictionBinding) {
    check_branches_within(c, MAX_ORDER_STATES)
}

/// [`check_branches_bound`], with the every-order search stopped at `bound`
/// distinct play states per branch instead of [`MAX_ORDER_STATES`].
pub fn check_branches_within(
    c: &Campaign,
    bound: usize,
) -> (Vec<Diagnostic>, ContradictionBinding) {
    let mut bind = ContradictionBinding {
        bound,
        ..ContradictionBinding::default()
    };
    let mut d = Vec::new();
    check_happenings(c, &mut d);
    let flow = Flow::new(c);
    check_undeclared_forks(c, &flow, &mut d);
    // The mainline's own skips (`DW0205`) are `crate::compiler::analyze`'s to report; here
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
        if r.world.is_some() {
            bind.branches += 1;
            bind.lines += r.chronicle.iter().filter(|l| l.kind != "ambient").count();
        }
        let walked = check_every_order(c, &flow, &r, bound, &mut d);
        bind.steps += walked.steps;
        bind.quiet += walked.quiet;
        bind.states += walked.states;
        bind.refused += walked.refused;
        bind.unproven += usize::from(walked.unproven);
        check_branch_skips(c, &flow, &r, &on_main, &mut d);
    }
    d.sort_by(|a, b| (&a.code, &a.path, &a.message).cmp(&(&b.code, &b.path, &b.message)));
    d.dedup_by(|a, b| a.code == b.code && a.path == b.path && a.message == b.message);
    (d, bind)
}

/// What `DW0485` examined across every realized branch — the binding count,
/// stated whether or not anything was found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContradictionBinding {
    /// Realized branches whose exported order was read.
    pub branches: usize,
    /// Dated chronicle lines across those exported orders.
    pub lines: usize,
    /// Steps on those branches' paths, summed.
    pub steps: usize,
    /// Of those, the quiet ones the search takes first.
    pub quiet: usize,
    /// Distinct play states the every-order search walked, summed.
    pub states: usize,
    /// Clashes those orders show that the exported orders do not.
    pub refused: usize,
    /// Branches whose search reached the bound — refused as unproven.
    pub unproven: usize,
    /// The bound, in distinct play states per branch.
    pub bound: usize,
}

impl ContradictionBinding {
    /// The binding line this check states on every run.
    pub fn line(&self) -> String {
        format!(
            "contradiction binding: {b} branch(es) read in their exported order over {l} dated \
             line(s); every legal order searched over {st} step(s) ({q} quiet), {s} distinct \
             play state(s) walked; {r} refused beyond the exported order (DW0485); {u} \
             branch(es) unproven at the bound of {n} state(s) per branch (DW0927)",
            b = self.branches,
            l = self.lines,
            st = self.steps,
            q = self.quiet,
            s = self.states,
            r = self.refused,
            u = self.unproven,
            n = self.bound,
        )
    }
}

/// `DW0205`, per branch. Optionality interacts with branches: a
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
            crate::compiler::flow::DW_OPTIONAL_GATES_MAINLINE,
            "quests",
            crate::compiler::analyze::objective_path(c, &s.objective),
            s.message(),
        ));
    }
}

/// `DW0481` — the forcing function. Every story node states what it does to the
/// story, or the campaign does not compile.
fn check_happenings(c: &Campaign, d: &mut Vec<Diagnostic>) {
    {
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
        // An ambush declares its beat ON THE AMBUSH, and the `spawn-actor` /
        // `unleash-actor` pairs it desugars into are that one beat lowered. The
        // author never wrote them and has no surface to reach them, so demanding
        // a `happening` from each is an obligation nobody can discharge — which
        // is what made `ambushes[]` uncompilable at 0.8.0 and above for as long
        // as the surface had existed. The declaration is checked here, once, on
        // the object that owns it.
        for (i, a) in c.quests.content.ambushes.iter().enumerate() {
            if a.happening.is_none() {
                d.push(missing(
                    "quests",
                    format!("/content/ambushes/{i}/happening"),
                    format!("ambush `{}`", a.id.as_str()),
                ));
            }
        }
        // Which generated beats those are. Keyed by (derived trigger id, actor)
        // rather than by position: an index into the desugared effect list would
        // have to track how many telegraph effects came first, and a telegraph
        // is authored and still owes its own declarations. A `spawn`/`unleash`
        // naming one of the ambush's own actors, inside the trigger that ambush
        // expands to, is exactly the generated set and nothing else.
        let derived: std::collections::HashSet<(String, String)> = c
            .quests
            .content
            .ambushes
            .iter()
            .flat_map(|a| {
                let tid = a.to_trigger().id.as_str().to_string();
                a.actors
                    .iter()
                    .map(move |act| (tid.clone(), act.as_str().to_string()))
            })
            .collect();
        let generated_by_an_ambush = |site: &delvewright_dsl::EffectSite, eff: &QuestEffect| {
            let delvewright_dsl::EffectSite::Trigger { trigger } = site else {
                return false;
            };
            let actor = match &eff.verb {
                Verb::SpawnActor { actor, .. } | Verb::UnleashActor { actor, .. } => actor.as_str(),
                _ => return false,
            };
            derived.contains(&(trigger.clone(), actor.to_string()))
        };

        let mut sites: Vec<(String, String)> = Vec::new();
        for_each_campaign_effect(c, &mut |path, site, eff| {
            if is_story_node(eff) && eff.happening.is_none() && !generated_by_an_ambush(site, eff) {
                sites.push((format!("{path}/happening"), eff.verb.tag().to_string()));
            }
        });
        for (path, verb) in sites {
            d.push(missing("quests", path, format!("the `{verb}` beat")));
        }
    }
    {
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
        if let Verb::SetFlag { flag, .. } = &eff.verb
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
                .filter(|(_, p)| crate::compiler::cast::selects(p, flags))
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
/// **Public because the chronicle is its whole input.** The subject each line
/// carries is derived (`QuestEffect::happening_subject`, spec-0071 §3), and the
/// only way to test that the derivation is what makes a contradiction visible is
/// to run this over a chronicle with the derived subject taken back out. A rule
/// whose perturbation cannot be reached is a rule nothing has ever tested.
///
/// Four rules, each decidable from the structured verbs alone:
/// 1. `dies(S)` then any later ACT by `S` — a dead man does nothing.
/// 2. `departs(S)` then a later beat by `S` with no `arrives(S)` between.
/// 3. `seals(S)` then a later beat about `S` that is not `opens(S)`.
/// 4. `loses(S)` then a later `loses(S)` with no `gains(S)` between.
///
/// Ambient lines (triggers, traps) are excluded: `flow` refuses to date them, so
/// ordering them against the dated account would invent a sequence.
///
/// This reads ONE order — the realized branch's exported one. The proof
/// [`check_branches`] runs asks the same question of every order the branch
/// admits ([`check_every_order`]).
pub fn check_contradictions(r: &RealizedBranch, d: &mut Vec<Diagnostic>) {
    for x in contradictions(&r.chronicle) {
        d.push(contradiction_diagnostic(r, &x, None));
    }
}

/// One clash the four rules find in one chronicle: the earlier line, the later
/// one, and what the later one does wrong.
struct Contradiction {
    subject: String,
    why: &'static str,
    prev: ChronicleLine,
    line: ChronicleLine,
}

impl Contradiction {
    /// The clash itself, independent of where in an order it was seen: the
    /// subject, the rule, and the two nodes.
    fn key(&self) -> (String, &'static str, String, String) {
        (
            self.subject.clone(),
            self.why,
            self.prev.line_node(),
            self.line.line_node(),
        )
    }
}

impl ChronicleLine {
    fn line_node(&self) -> String {
        format!("{} {}", self.kind, self.node)
    }
}

/// Does a beat with this verb ACT? `learns`/`believes` are EPISTEMIC: their
/// subject is what the beat is *about*, not somebody acting, and a living
/// character may perfectly well believe something about a dead one. They are
/// therefore never contradictions — "Elpenor mourns a man standing beside him"
/// is exactly the class spec-0025 leaves to the chronicle's human reader,
/// because no verb makes it decidable.
fn acts(v: HappeningVerb) -> bool {
    !matches!(v, HappeningVerb::Learns | HappeningVerb::Believes)
}

/// Does a beat with this verb leave its subject in a state a later beat can
/// contradict?
fn carries(v: HappeningVerb) -> bool {
    matches!(
        v,
        HappeningVerb::Dies | HappeningVerb::Departs | HappeningVerb::Seals | HappeningVerb::Loses
    )
}

/// The four rules: what an acting beat `next` does wrong when the last acting
/// beat about the same subject was `prev`.
fn clash(prev: HappeningVerb, next: HappeningVerb) -> Option<&'static str> {
    match prev {
        HappeningVerb::Dies => Some("acts after it dies"),
        HappeningVerb::Departs if next != HappeningVerb::Arrives => {
            Some("acts while it is offstage")
        }
        HappeningVerb::Seals if next != HappeningVerb::Opens => Some("is used after it is sealed"),
        HappeningVerb::Loses if next == HappeningVerb::Loses => Some("is spent twice over"),
        _ => None,
    }
}

/// The four rules, read one line at a time — the one reading both the exported
/// chronicle ([`contradictions`]) and every order the search walks
/// ([`check_every_order`]) go through. Per subject it holds the last acting
/// line about it, when that line leaves the subject in a state a later line
/// can contradict.
#[derive(Clone, Default)]
struct ClashTracker {
    last: BTreeMap<String, ChronicleLine>,
}

impl ClashTracker {
    /// Read `l`, the next line of the chronicle: the clash it completes, if it
    /// completes one.
    fn read(&mut self, l: &ChronicleLine) -> Option<Contradiction> {
        if l.kind == "ambient" || !acts(l.verb) {
            return None;
        }
        let subject = l.subject.as_deref()?;
        let hit = self.last.get(subject).and_then(|prev| {
            clash(prev.verb, l.verb).map(|why| Contradiction {
                subject: subject.to_string(),
                why,
                prev: prev.clone(),
                line: l.clone(),
            })
        });
        if carries(l.verb) {
            self.last.insert(subject.to_string(), l.clone());
        } else {
            self.last.remove(subject);
        }
        hit
    }
}

/// The four rules over one chronicle, in its order.
fn contradictions(chronicle: &[ChronicleLine]) -> Vec<Contradiction> {
    let mut t = ClashTracker::default();
    chronicle.iter().filter_map(|l| t.read(l)).collect()
}

/// The `DW0485` diagnostic for one clash. `order` is `None` when the clash is in
/// the branch's exported order, and otherwise the objectives of the other legal
/// order it was found in, in walk order.
fn contradiction_diagnostic(
    r: &RealizedBranch,
    x: &Contradiction,
    order: Option<&[String]>,
) -> Diagnostic {
    let (on, prescription) = match order {
        None => (
            String::new(),
            "Prescription: fix whichever beat is on the wrong branch — usually the later one \
             belongs to the sibling branch and needs its flag gate."
                .to_string(),
        ),
        Some(order) => (
            format!(
                ", in a play order the branch admits other than the exported one ({})",
                render_order(order)
            ),
            "The exported order does not show it, and a player is not bound to the exported \
             order: every step of that order is legal where it stands. Prescription: make the \
             later beat unable to follow the earlier one — an `after` edge, a quest-complete \
             trigger, or a `forbids_flags` on a flag the earlier beat's bundle sets closes the \
             strand before it — or fix whichever beat is on the wrong branch."
                .to_string(),
        ),
    };
    Diagnostic::error(
        DW_BRANCH_CONTRADICTION,
        "quests",
        format!("/content/quests#branch/{}", r.branch.id),
        format!(
            "on branch `{}`{on}, `{}` {}:\n    #{} [{}] {} — {}\n    #{} [{}] {} — {}\nBranch \
             assignment: {}. The two lines cannot both be true of one playthrough. \
             {prescription} Do NOT reword the `happening` to hide the clash: the verbs are the \
             only part of the chronicle a machine can check",
            r.branch.id,
            x.subject,
            x.why,
            x.prev.n,
            verb_name(x.prev.verb),
            x.prev.node,
            x.prev.text,
            x.line.n,
            verb_name(x.line.verb),
            x.line.node,
            x.line.text,
            assignment(&r.branch),
        ),
    )
}

fn render_order(order: &[String]) -> String {
    order
        .iter()
        .map(|o| format!("`{o}`"))
        .collect::<Vec<_>>()
        .join(" → ")
}

/// The most distinct play states [`check_every_order`] walks on one branch. A
/// branch whose legal orders reach more is refused as unproven, by name — never
/// called clean. Sized to time: measured on a branch of twenty freely
/// interleaved strands (45 steps, 20 quiet), a 100 000-state run took 9.4 s on
/// the dev profile, so this bound costs a refused branch about five seconds.
pub const MAX_ORDER_STATES: usize = 50_000;

/// `DW0485` over **every order the branch admits**, not only the exported one.
///
/// **The quantifier.** A *legal order* of a branch is a sequence of distinct
/// steps of the branch's own path (every objective of every quest its world
/// completes, each `talk-to` with the option the world takes) in which each step
/// passes the replay's per-step test where it stands ([`Flow::walk`]: quest
/// active, `after` done, `requires_flags` held, `forbids_flags` clear, the
/// completing option reachable) and no step follows the one that fires
/// `campaign-complete`. Its chronicle is written by the same [`Chronicler`] the
/// exported chronicle is, every effect gate read against the state that order
/// has reached. The branch is refused when the four rules ([`ClashTracker`])
/// refuse the chronicle of ANY legal order — any prefix of one, since a player
/// sees a clash the moment the second line plays.
///
/// **The search is exact.** It walks every legal order from the start of the
/// delve, depth first, and merges two orders that reach the same *play state*:
/// the objectives done, the flags held, the value of every datum an effect gate
/// compares, and, per subject some step can leave in a carried state, the last
/// acting line about it while that line carries. Everything a later step's
/// legality, its fired effects and its lines' verdict depends on is in that
/// state (quest activity and completion are functions of the objectives done,
/// a quest's own line plays at its first step done), so two orders that reach
/// one state have the same continuations, with the same clashes. Nothing about
/// `forbids_flags` or effect gates is assumed: each order reads every gate where
/// it stands.
///
/// **One reduction, and why it loses nothing.** A step is *quiet*
/// ([`quiet_steps`]) when it fires the same effects whenever it is taken (no
/// gated effect in its bundle or its quest's `on_complete`), sets no flag any
/// `forbids_flags` or effect gate reads and writes no datum any effect gate
/// compares, does not end the delve, and has no acting line about a subject
/// some step can leave in a carried state. Where a quiet step is legal, the
/// search walks only it. Take any legal order `O` from that state. The order
/// that takes the quiet step first and then `O` without it is legal: the quiet
/// step only adds objectives, active quests and flags that only `requires`
/// gates read, so no step of `O` is shut by it. Every other step fires what it
/// fires in `O`, since no gate it reads moved. The lines the rules read come in
/// the same order, since the quiet step's own lines — and its quest's line and
/// completion lines, which may now play at a different step — act on no subject
/// a rule tracks. So any clash `O` shows, that order shows too. Whether other
/// steps can shut the quiet step does not matter: it is taken while it is open.
///
/// **Cost.** Every visited state checks every step's legality once:
/// `O(states × steps)` legality tests, plus one chronicle clone per edge.
/// `states` is bounded by the downsets of the steps that are not quiet times the
/// flag and carried-line variants they produce — exponential in how many of
/// them a player may interleave freely, linear in the rest. The search stops at
/// [`MAX_ORDER_STATES`] distinct states, and a branch that reaches the bound is
/// refused as **unproven** (`DW0927`), naming the bound: a bound hit is never a
/// pass.
///
/// Every order a refusal prints is one the search walked, so it is a play order
/// a player can walk.
fn check_every_order(
    c: &Campaign,
    flow: &Flow<'_>,
    r: &RealizedBranch,
    bound: usize,
    d: &mut Vec<Diagnostic>,
) -> OrderWalk {
    let (found, stats) = search_orders(c, flow, r, bound);
    for (x, order) in &found {
        d.push(contradiction_diagnostic(r, x, Some(order)));
    }
    if stats.unproven {
        d.push(unproven_diagnostic(r, bound, stats.steps, stats.quiet));
    }
    stats
}

/// The search [`check_every_order`] runs: every clash a legal order shows that
/// the exported order does not, each with the first order found showing it.
fn search_orders(
    c: &Campaign,
    flow: &Flow<'_>,
    r: &RealizedBranch,
    bound: usize,
) -> (Vec<(Contradiction, Vec<String>)>, OrderWalk) {
    let mut stats = OrderWalk::default();
    let Some(w) = r.world else {
        return (Vec::new(), stats);
    };
    let steps = flow.playthrough_in(w).steps;
    let beats: Vec<Vec<(HappeningVerb, String)>> =
        steps.iter().map(|s| possible_beats(c, s)).collect();
    let carried: BTreeSet<String> = beats
        .iter()
        .flatten()
        .filter(|(v, _)| carries(*v))
        .map(|(_, s)| s.clone())
        .collect();
    let quiet = quiet_steps(c, flow, &steps, &beats, &carried);
    let read = data_gates_read(c);
    let mut search = OrderSearch {
        steps: &steps,
        quiet: &quiet,
        carried: &carried,
        read: &read,
        bound,
        visited: BTreeSet::new(),
        flag_ids: BTreeMap::new(),
        line_ids: BTreeMap::new(),
        seen: contradictions(&r.chronicle)
            .iter()
            .map(Contradiction::key)
            .collect(),
        found: Vec::new(),
        hit: false,
    };
    search.explore(&OrderNode {
        walk: flow.walk(),
        ch: Chronicler::new(c),
        tracker: ClashTracker::default(),
        order: Vec::new(),
    });
    stats.states = search.visited.len();
    stats.quiet = quiet.iter().filter(|q| **q).count();
    stats.steps = steps.len();
    stats.refused = search.found.len();
    stats.unproven = search.hit;
    (search.found, stats)
}

/// **The second method, for tests.** Per realized branch, every clash some
/// legal order shows, two ways: what `DW0485` reads (the exported order, then
/// [`search_orders`]), and every legal order walked one at a time with no
/// merged states and no quiet steps — the two things the search does that
/// could lose an order. Each clash reads `branch | subject | rule | earlier
/// line | later line`. The enumeration is exponential in the steps: a
/// fixture's handful only.
#[doc(hidden)]
pub fn clashes_by_search_and_by_enumeration(c: &Campaign) -> (BTreeSet<String>, BTreeSet<String>) {
    fn named(branch: &str, k: (String, &'static str, String, String)) -> String {
        format!("{branch} | {} | {} | {} | {}", k.0, k.1, k.2, k.3)
    }
    fn every<'f, 'a, 'c>(
        steps: &[PathStep],
        n: &OrderNode<'f, 'a, 'c>,
        out: &mut BTreeSet<(String, &'static str, String, String)>,
    ) {
        for step in steps {
            if !n.walk.legal(step) {
                continue;
            }
            let mut walk = n.walk.clone();
            let j = walk.take(step);
            let mut ch = n.ch.clone();
            ch.push_step(&j);
            let mut tracker = n.tracker.clone();
            for l in &ch.take_lines() {
                if let Some(x) = tracker.read(l) {
                    out.insert(x.key());
                }
            }
            if walk.ended() {
                continue;
            }
            every(
                steps,
                &OrderNode {
                    walk,
                    ch,
                    tracker,
                    order: Vec::new(),
                },
                out,
            );
        }
    }
    let flow = Flow::new(c);
    let (mut searched, mut enumerated) = (BTreeSet::new(), BTreeSet::new());
    for b in enumerate(c) {
        let r = realize_one(c, &flow, b);
        let Some(w) = r.world else { continue };
        let id = r.branch.id.clone();
        for x in contradictions(&r.chronicle) {
            searched.insert(named(&id, x.key()));
        }
        for (x, _) in search_orders(c, &flow, &r, MAX_ORDER_STATES).0 {
            searched.insert(named(&id, x.key()));
        }
        let steps = flow.playthrough_in(w).steps;
        let mut all = BTreeSet::new();
        every(
            &steps,
            &OrderNode {
                walk: flow.walk(),
                ch: Chronicler::new(c),
                tracker: ClashTracker::default(),
                order: Vec::new(),
            },
            &mut all,
        );
        enumerated.extend(all.into_iter().map(|k| named(&id, k)));
    }
    (searched, enumerated)
}

/// `DW0927`: the branch is refused as unproven — the search reached its bound.
fn unproven_diagnostic(r: &RealizedBranch, bound: usize, steps: usize, quiet: usize) -> Diagnostic {
    Diagnostic::error(
        DW_BRANCH_CONTRADICTION_UNPROVEN,
        "quests",
        format!("/content/quests#branch/{}", r.branch.id),
        format!(
            "on branch `{}`, whether any play order shows a hard event contradiction is \
             UNPROVEN: the every-order search reached its bound of {bound} distinct play states \
             before it had walked every legal order ({steps} step(s) on the branch's path, \
             {quiet} of them quiet). The branch is refused rather than called clean — an order \
             the search never reached may carry a clash. Branch assignment: {}. Prescription: \
             cut the number of orders a player can interleave — chain strands that need not run \
             side by side (an `after` edge, a quest-complete trigger), or drop a gate that makes \
             a beat's effects depend on the order (a `forbids_flags`, or an effect gate on a \
             flag several strands set). Do NOT raise the bound to get green",
            r.branch.id,
            assignment(&r.branch),
        ),
    )
}

/// What [`check_every_order`] examined on one branch.
#[derive(Clone, Copy, Debug, Default)]
pub struct OrderWalk {
    /// Steps on the branch's path.
    pub steps: usize,
    /// Of those, the quiet ones ([`quiet_steps`]).
    pub quiet: usize,
    /// Distinct play states the search walked.
    pub states: usize,
    /// Clashes found in other legal orders that the exported order does not show.
    pub refused: usize,
    /// The search reached its bound.
    pub unproven: bool,
}

/// One node of the every-order search: a legal order so far, its walk and its
/// chronicle.
#[derive(Clone)]
struct OrderNode<'f, 'a, 'c> {
    walk: crate::compiler::flow::Walk<'f, 'a>,
    ch: Chronicler<'c>,
    tracker: ClashTracker,
    order: Vec<String>,
}

/// The play state two orders are merged on ([`check_every_order`]): the steps
/// done, the flags held (interned), the value of every datum a gate reads, and
/// per carried subject the last carrying line about it (interned).
type PlayState = (Vec<u64>, Vec<usize>, Vec<Option<i64>>, Vec<(usize, usize)>);

struct OrderSearch<'s> {
    steps: &'s [PathStep],
    quiet: &'s [bool],
    carried: &'s BTreeSet<String>,
    read: &'s BTreeSet<String>,
    bound: usize,
    visited: BTreeSet<PlayState>,
    flag_ids: BTreeMap<String, usize>,
    line_ids: BTreeMap<String, usize>,
    seen: BTreeSet<(String, &'static str, String, String)>,
    found: Vec<(Contradiction, Vec<String>)>,
    hit: bool,
}

impl OrderSearch<'_> {
    fn state(&mut self, n: &OrderNode<'_, '_, '_>) -> PlayState {
        let mut done = vec![0u64; self.steps.len().div_ceil(64)];
        for (i, s) in self.steps.iter().enumerate() {
            if n.walk.done(&s.objective) {
                done[i / 64] |= 1 << (i % 64);
            }
        }
        let mut flags: Vec<usize> = Vec::new();
        for f in n.walk.flags() {
            let next = self.flag_ids.len();
            flags.push(*self.flag_ids.entry(f.clone()).or_insert(next));
        }
        flags.sort_unstable();
        let data: Vec<Option<i64>> = n
            .walk
            .data()
            .into_iter()
            .filter(|(id, _)| self.read.contains(*id))
            .map(|(_, v)| v)
            .collect();
        let mut lines: Vec<(usize, usize)> = Vec::new();
        for (subject, l) in &n.tracker.last {
            if !self.carried.contains(subject) {
                continue;
            }
            let s = {
                let next = self.line_ids.len();
                *self
                    .line_ids
                    .entry(format!("subject {subject}"))
                    .or_insert(next)
            };
            let k = {
                let next = self.line_ids.len();
                *self.line_ids.entry(l.line_node()).or_insert(next)
            };
            lines.push((s, k));
        }
        (done, flags, data, lines)
    }

    /// `n` with step `i` taken, its new lines read by the rules; every clash
    /// they complete that no earlier order showed is kept with this order.
    fn child<'f, 'a, 'c>(&mut self, n: &OrderNode<'f, 'a, 'c>, i: usize) -> OrderNode<'f, 'a, 'c> {
        let mut walk = n.walk.clone();
        let j = walk.take(&self.steps[i]);
        let mut ch = n.ch.clone();
        ch.push_step(&j);
        let mut tracker = n.tracker.clone();
        let mut order = n.order.clone();
        order.push(self.steps[i].objective.clone());
        for l in &ch.take_lines() {
            if let Some(x) = tracker.read(l)
                && self.seen.insert(x.key())
            {
                self.found.push((x, order.clone()));
            }
        }
        OrderNode {
            walk,
            ch,
            tracker,
            order,
        }
    }

    fn explore(&mut self, n: &OrderNode<'_, '_, '_>) {
        if self.hit {
            return;
        }
        let key = self.state(n);
        if !self.visited.insert(key) {
            return;
        }
        if self.visited.len() > self.bound {
            self.hit = true;
            return;
        }
        if let Some(i) =
            (0..self.steps.len()).find(|&i| self.quiet[i] && n.walk.legal(&self.steps[i]))
        {
            let next = self.child(n, i);
            self.explore(&next);
            return;
        }
        for i in 0..self.steps.len() {
            if !n.walk.legal(&self.steps[i]) {
                continue;
            }
            let next = self.child(n, i);
            if next.walk.ended() {
                continue;
            }
            self.explore(&next);
        }
    }
}

/// Every effect of `effs`, nested lists included, gated or not.
fn each_effect<'e>(effs: &'e [QuestEffect], f: &mut dyn FnMut(&'e QuestEffect)) {
    for e in effs {
        f(e);
        for list in e.nested_effect_lists() {
            each_effect(list, f);
        }
    }
}

/// The data some effect gate compares (`requires_state`): the only reason a
/// datum's value can change what a step fires.
fn data_gates_read(c: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for_each_campaign_effect(c, &mut |_, _, e| {
        for cmp in e.requires_state() {
            out.insert(cmp.state.as_str().to_string());
        }
    });
    out
}

/// Which steps are **quiet** — the steps [`check_every_order`] may take first
/// without losing an order. A step is quiet when all of these hold:
///
/// - it fires the same effects whenever it is taken: no effect in its
///   objective's bundle or its quest's `on_complete` carries a flag or numeric
///   gate;
/// - it changes no gate another step reads: it sets no flag (by its option,
///   its bundle, its quest's `on_complete`) that any `forbids_flags` anywhere or
///   any effect's `requires_flags` reads, directly or through an ambient
///   producer, and writes no datum any effect gate compares;
/// - it does not end the delve;
/// - none of its possible lines acts on a subject some step of the path can
///   leave in a carried state (`carried`).
fn quiet_steps(
    c: &Campaign,
    flow: &Flow<'_>,
    steps: &[PathStep],
    beats: &[Vec<(HappeningVerb, String)>],
    carried: &BTreeSet<String>,
) -> Vec<bool> {
    // Flags whose value some gate reads in a way that is not "more is more".
    let mut gates: BTreeSet<String> = BTreeSet::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            gates.extend(o.forbids_flags().iter().map(|f| f.as_str().to_string()));
        }
    }
    for t in &c.dialogue.content.dialogues {
        for n in &t.nodes {
            for o in &n.options {
                gates.extend(o.forbids_flags.iter().map(|f| f.as_str().to_string()));
            }
        }
    }
    for_each_campaign_effect(c, &mut |_, _, e| {
        gates.extend(e.requires_flags().iter().map(|f| f.as_str().to_string()));
        gates.extend(e.forbids_flags().iter().map(|f| f.as_str().to_string()));
    });
    // A flag that lets an ambient producer set a gate flag reaches the gate.
    loop {
        let mut changed = false;
        for (flag, requires) in flow.ambient_producers() {
            if gates.contains(flag) {
                for r in requires {
                    changed |= gates.insert(r.clone());
                }
            }
        }
        if !changed {
            break;
        }
    }
    let read = data_gates_read(c);
    steps
        .iter()
        .zip(beats)
        .map(|(step, beats)| {
            let Some(q) = c
                .quests
                .content
                .quests
                .iter()
                .find(|q| q.id.as_str() == step.quest)
            else {
                return false;
            };
            let mut sets: Vec<String> = Vec::new();
            if let Some(n) = step.talk_option
                && let Some(npc) = talk_to_npc(c, &step.objective)
                && let Some((_, opt)) = option_at(c, npc, n)
            {
                for e in &opt.effects {
                    if let delvewright_dsl::DialogueEffect::SetFlag { flag } = e {
                        sets.push(flag.as_str().to_string());
                    }
                }
            }
            let mut steady = true;
            let mut bundle = |e: &QuestEffect| {
                if !e.requires_flags().is_empty()
                    || !e.forbids_flags().is_empty()
                    || !e.requires_state().is_empty()
                    || matches!(e.verb, Verb::CampaignComplete { .. })
                    || e.writes_state()
                        .is_some_and(|(id, _)| read.contains(id.as_str()))
                {
                    steady = false;
                }
                if let Verb::SetFlag { flag, .. } = &e.verb {
                    sets.push(flag.as_str().to_string());
                }
            };
            if let Some(effs) = q
                .on_objective_complete
                .get(&delvewright_dsl::ObjectiveId(step.objective.clone()))
            {
                each_effect(effs, &mut bundle);
            }
            each_effect(&q.on_complete, &mut bundle);
            steady
                && !sets.iter().any(|f| gates.contains(f))
                && !beats.iter().any(|(v, s)| acts(*v) && carried.contains(s))
        })
        .collect()
}

/// Every `(verb, subject)` a step can put in the dated chronicle, whatever the
/// order: its quest's own line, its objective's, the option it takes, and every
/// effect its objective bundle or its quest's `on_complete` declares, gated or
/// not. A superset: it decides which subjects the rules track and which steps
/// are quiet ([`quiet_steps`]), and a superset can only make fewer steps quiet.
fn possible_beats(c: &Campaign, step: &PathStep) -> Vec<(HappeningVerb, String)> {
    let mut out = Vec::new();
    let Some(q) = c
        .quests
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == step.quest)
    else {
        return out;
    };
    let mut stated = |h: Option<&Happening>| {
        if let Some(h) = h
            && let Some(s) = &h.subject
        {
            out.push((h.verb, s.to_string()));
        }
    };
    stated(q.happening.as_ref());
    if let Some(o) = q
        .objectives
        .iter()
        .find(|o| o.id().as_str() == step.objective)
    {
        stated(o.happening());
    }
    if let Some(n) = step.talk_option
        && let Some(npc) = talk_to_npc(c, &step.objective)
        && let Some((_, opt)) = option_at(c, npc, n)
    {
        stated(opt.happening.as_ref());
    }
    fn effects(effs: &[QuestEffect], out: &mut Vec<(HappeningVerb, String)>) {
        for e in effs {
            if let Some(h) = &e.happening
                && let Some(s) = e.happening_subject()
            {
                out.push((h.verb, s.id.to_string()));
            }
            for list in e.nested_effect_lists() {
                effects(list, out);
            }
        }
    }
    if let Some(effs) = q
        .on_objective_complete
        .get(&delvewright_dsl::ObjectiveId(step.objective.clone()))
    {
        effects(effs, &mut out);
    }
    effects(&q.on_complete, &mut out);
    out.sort();
    out.dedup();
    out
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
