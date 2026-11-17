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
//! | `DW0481` | **Missing `happening`** — a story node that never said what it does to the story. The forcing function. |
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

delvewright_dsl::dw_code! {
    /// A flag forks casts / staging / structure but belongs to no declared branch point.
    pub const DW_FORK_UNDECLARED: DwCode = DwCode::new("DW0480", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// A story node carries no `happening` declaration (DSL v0.8+).
    pub const DW_HAPPENING_MISSING: DwCode = DwCode::new("DW0481", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// A declared branch reaches no ending — or not the one it declares.
    pub const DW_BRANCH_TERMINAL: DwCode = DwCode::new("DW0482", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// A quest's cast selector does not resolve to exactly one placement on a branch.
    pub const DW_BRANCH_CAST: DwCode = DwCode::new("DW0483", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// Branch-exclusive content is reachable under a sibling branch's assignment.
    pub const DW_BRANCH_LEAKAGE: DwCode = DwCode::new("DW0484", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// Two chronicle lines on one branch contradict each other.
    pub const DW_BRANCH_CONTRADICTION: DwCode = DwCode::new("DW0485", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// Whether any play order of a branch shows a `DW0485` clash is unproven: the
    /// every-order search reached [`MAX_ORDER_STATES`].
    pub const DW_BRANCH_CONTRADICTION_UNPROVEN: DwCode = DwCode::new("DW0927", ExitTier::Build);
}

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

/// The elective acts the story world `pt` walks does not perform, by id —
/// empty for a playthrough that is no story world's.
fn skipped_acts(flow: &Flow<'_>, pt: &crate::compiler::flow::Playthrough) -> BTreeSet<String> {
    let Some(ctx) = &pt.story else {
        return BTreeSet::new();
    };
    flow.story()
        .electives
        .iter()
        .map(|e| e.id.clone())
        .filter(|id| !ctx.performs.contains(id))
        .collect()
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
    // Story worlds in canonical order (spec-0108 §4.2): per selection the
    // *all* world first, so a branch every world of its selection realizes keeps
    // today's world and path.
    let world = (0..flow.story_count()).find(|&i| {
        let f = flow.story_flags(i);
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
    let pt = flow.story_playthrough(w);
    let journal = flow.journal(&pt);
    let (chronicle, endings) = chronicle_of(c, &journal, &skipped_acts(flow, &pt));
    let entry_choices = entry_choices(c, &pt, &branch.set);
    RealizedBranch {
        branch,
        world: Some(w),
        path: pt.steps,
        entry_choices,
        chronicle,
        endings,
        completed: flow.story_completed(w),
    }
}

/// The dialogue choices that ENTER this branch: every `talk-to` option on the
/// path that sets one of the branch's pinned-set flags — an objective's talk,
/// or a story world's choice.
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
    // A story world's choices (`flow::StoryCtx::choices`): an option that
    // completes no objective, taken by its own `talk-to` step.
    for (_, ch) in pt.story.iter().flat_map(|ctx| ctx.choices.iter()) {
        if ch.sets.iter().any(|f| set.contains(f)) {
            out.push(EntryChoice {
                npc: ch.npc.clone(),
                option: ch.n,
                command: format!(
                    "/trigger {} set {}",
                    crate::compiler::plan::dlg_trigger(&ch.npc),
                    ch.n
                ),
            });
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
///
/// `skipped` are the elective acts the story world does not perform (empty
/// outside one). An elective ambient act the world performs as an entry deed is
/// dated where it is performed — a `deed` line in front of that step — and one
/// it skips has no line at all; neither is listed among the undated ambient
/// beats, which keep every other trigger and trap beat.
fn chronicle_of(
    c: &Campaign,
    journal: &[JournalStep],
    skipped: &BTreeSet<String>,
) -> (Vec<ChronicleLine>, Vec<String>) {
    let mut w = Chronicler::new(c);
    w.not_ambient = skipped.clone();
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
    /// The acts never listed among the ambient beats: the elective acts the
    /// story world skips, and every act it dated as a `deed`.
    not_ambient: BTreeSet<String>,
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
            not_ambient: BTreeSet::new(),
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
        // A story world's dialogue choices land in front of the step they are
        // taken at, ahead of its deeds, as the walk takes them
        // (`flow::Walk::take`): an option that completes no objective and
        // declares a `happening` is a dated `choice` line, named as the option
        // that completes a `talk-to` is.
        for (npc, n) in &step.choices {
            if let Some((node, opt)) = option_at(c, npc, *n)
                && let Some(h) = &opt.happening
            {
                self.push("choice", format!("{npc} {node}#{n}"), h, h.subject.clone());
            }
        }
        // A story world's entry deeds land in front of the step they are
        // performed at (spec-0108 §4.1): each fired effect of the act's bundle
        // that declares a `happening` is a dated `deed` line.
        if !step.deeds.is_empty() {
            let mut by_path: BTreeMap<String, &QuestEffect> = BTreeMap::new();
            for_each_campaign_effect(c, &mut |path, _, eff| {
                by_path.insert(path.to_string(), eff);
            });
            for (act, fired) in &step.deeds {
                self.not_ambient.insert(act.clone());
                for path in fired {
                    let Some(eff) = by_path.get(path) else {
                        continue;
                    };
                    if let Some(h) = eff.happening.as_ref() {
                        let subject = eff.happening_subject().map(|s| s.id.to_string());
                        self.push("deed", path.clone(), h, subject);
                    }
                }
            }
        }
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
        let not_ambient = &self.not_ambient;
        for_each_campaign_effect(self.c, &mut |path, site, eff| {
            let act = match site {
                EffectSite::Trigger { trigger } => trigger,
                EffectSite::Trap { trap } => trap,
                _ => return,
            };
            if not_ambient.contains(act.as_str()) {
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
/// `happening`?
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
    bind.search = flow.story().branch_search.clone();
    for b in enumerate(c) {
        let r = realize_one(c, &flow, b);
        check_leakage(c, &flow, &r, &mut d);
        check_terminality(
            c,
            &r,
            flow.story().branch_search.branches.get(&r.branch.id),
            &mut d,
        );
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
        bind.stubborn += walked.stubborn;
        if r.world.is_some() {
            bind.per_branch.push((r.branch.id.clone(), walked));
        }
        bind.refused += walked.refused;
        bind.unproven += usize::from(walked.unproven);
    }
    d.sort_by(|a, b| (&a.code, &a.path, &a.message).cmp(&(&b.code, &b.path, &b.message)));
    d.dedup_by(|a, b| a.code == b.code && a.path == b.path && a.message == b.message);
    (d, bind)
}

/// What `DW0485` examined across every realized branch — the binding count,
/// stated whether or not anything was found — and what the branch search ran
/// for the branches no first world realized (`DW0482`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
    /// Of those, the states where the stubborn set left a legal step out.
    pub stubborn: usize,
    /// Clashes those orders show that the exported orders do not.
    pub refused: usize,
    /// Branches whose search reached the bound — refused as unproven.
    pub unproven: usize,
    /// The bound, in distinct play states per branch.
    pub bound: usize,
    /// What the every-order search examined, per realized branch, by id.
    pub per_branch: Vec<(String, OrderWalk)>,
    /// What the branch search ran ([`crate::compiler::flow::BranchSearch`]).
    pub search: crate::compiler::flow::BranchSearch,
}

impl ContradictionBinding {
    /// The binding line this check states on every run.
    pub fn line(&self) -> String {
        format!(
            "contradiction binding: {b} branch(es) read in their exported order over {l} dated \
             line(s); every legal order searched over {st} step(s) ({q} quiet), {s} distinct \
             play state(s) walked (a stubborn set left a legal step out at {k}); {r} refused beyond the exported order (DW0485); {u} \
             branch(es) unproven at the bound of {n} state(s) per branch (DW0927)",
            b = self.branches,
            l = self.lines,
            st = self.steps,
            q = self.quiet,
            s = self.states,
            k = self.stubborn,
            r = self.refused,
            u = self.unproven,
            n = self.bound,
        )
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
        // is what made `ambushes[]` uncompilable for as long
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
                let tid = a
                    .to_trigger(&c.quests.content.actors)
                    .id
                    .as_str()
                    .to_string();
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

    // Every story world (spec-0108 §4.4): a flag only elective acts write is
    // set in the *all* world and not in the *none* world, so a deed-made fork is
    // a fork here too.
    let worlds: Vec<BTreeSet<String>> = (0..flow.story_count())
        .map(|i| flow.story_flags(i))
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
    let candidates: Vec<usize> = (0..flow.story_count())
        .filter(|&i| {
            let f = flow.story_flags(i);
            r.branch.set.iter().all(|x| f.contains(x))
        })
        .collect();
    if candidates.is_empty() {
        // Not reachable at all — `DW0482` owns that.
        return;
    }
    let mut leaked: BTreeSet<String> = BTreeSet::new();
    for i in candidates {
        let f = flow.story_flags(i);
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
                "no `set-flag` found — the flag is a disarm's `sets_flag`, an act a world \
                 performs or skips"
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
fn check_terminality(
    c: &Campaign,
    r: &RealizedBranch,
    searched: Option<&crate::compiler::flow::SearchedBranch>,
    d: &mut Vec<Diagnostic>,
) {
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
                 campaign sets {} while leaving {} unset{}. A declared branch nobody can take is a \
                 branch nothing proves. Branch assignment: {}. Prescription: give the fork a \
                 dialogue option (or a beat) that really sets this branch's flags, or drop the \
                 branch from `{}`",
                r.branch.id,
                join(&r.branch.set),
                join(&r.branch.unset),
                match searched {
                    Some(s) => format!(
                        " — the branch search ran {} participation(s) over {} dialogue \
                         selection(s): every subset of the {} elective act(s) its flags turn on \
                         ({}), on the all and the none world",
                        s.runs,
                        s.selections,
                        s.acts.len(),
                        if s.acts.is_empty() {
                            "none".to_string()
                        } else {
                            s.acts.join(", ")
                        },
                    ),
                    None => String::new(),
                },
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
    let pt = flow.story_playthrough(w);
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
/// **Two reductions, and why they lose nothing.** A step is *quiet*
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
/// Elsewhere the search walks the legal steps of a **stubborn set**
/// ([`Footprints::stubborn`]; `docs/reference/order-reduction.md`). Take any
/// legal order `O` from the state. If `O` takes a step of the set, the first
/// one it takes was legal here (no step before it is in the set, so none is in
/// its necessary enabling set) and conflicts with none before it, so it can be
/// taken first with the rest in their order: the same state, the same lines,
/// the same verdicts. If `O` takes none, a legal step of the set conflicts
/// with every step of `O` not at all, and taking it first keeps `O` legal and
/// its verdicts the same. Either way an order that starts with a walked step
/// shows every clash `O` shows, and the argument repeats from the next state;
/// the states are acyclic, since every step settles an objective.
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

/// What the every-order search reads off one story world, once: its path's
/// steps, the beats each can write, the subjects the rules track, the quiet
/// steps, the data a gate compares and the steps' footprints.
struct OrderModel {
    pt: crate::compiler::flow::Playthrough,
    steps: Vec<PathStep>,
    carried: BTreeSet<String>,
    quiet: Vec<bool>,
    read: BTreeSet<String>,
    fp: Footprints,
}

impl OrderModel {
    fn new(c: &Campaign, flow: &Flow<'_>, w: usize) -> Self {
        let pt = flow.story_playthrough(w);
        let steps = pt.steps.clone();
        // The entry deeds and choices due in front of each step: they travel with
        // it in every order (`Walk::take`), so their lines are its lines.
        let due: Vec<Vec<&[QuestEffect]>> = steps
            .iter()
            .map(|s| {
                pt.story
                    .as_ref()
                    .map(|ctx| {
                        ctx.deeds_before(&s.objective)
                            .map(|a| flow.act_effects(a))
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect();
        let beats: Vec<Vec<(HappeningVerb, String)>> = steps
            .iter()
            .zip(&due)
            .map(|(s, d)| possible_beats(c, s, d, true))
            .collect();
        // A subject a clash can involve: some step can leave it in a carried
        // state, and acting lines about it stand at two steps or more — a subject
        // only one step acts on has no later line to clash with in any order but
        // the ones that step's own lines already fix.
        let mut acted_at: BTreeMap<&str, usize> = BTreeMap::new();
        for b in &beats {
            let at: BTreeSet<&str> = b
                .iter()
                .filter(|(v, _)| acts(*v))
                .map(|(_, s)| s.as_str())
                .collect();
            for s in at {
                *acted_at.entry(s).or_default() += 1;
            }
        }
        let carried: BTreeSet<String> = beats
            .iter()
            .flatten()
            .filter(|(v, s)| carries(*v) && acted_at.get(s.as_str()).is_some_and(|n| *n > 1))
            .map(|(_, s)| s.clone())
            .collect();
        let quiet = quiet_steps(c, flow, &steps, &beats, &carried, pt.story.as_deref());
        let mut read = data_gates_read(c);
        for (_, a) in pt.story.iter().flat_map(|ctx| &ctx.deeds) {
            read.extend(flow.act_gate_reads(a).1);
        }
        let fp = Footprints::new(c, flow, &steps, &carried, &read, pt.story.as_deref());
        OrderModel {
            pt,
            steps,
            carried,
            quiet,
            read,
            fp,
        }
    }
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
    let m = OrderModel::new(c, flow, w);
    let mut search = OrderSearch {
        steps: &m.steps,
        quiet: &m.quiet,
        fp: &m.fp,
        stubborn: 0,
        carried: &m.carried,
        read: &m.read,
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
        walk: flow.walk_for(&m.pt),
        ch: Chronicler::new(c),
        tracker: ClashTracker::default(),
        order: Vec::new(),
    });
    stats.states = search.visited.len();
    stats.quiet = m.quiet.iter().filter(|q| **q).count();
    stats.steps = m.steps.len();
    stats.stubborn = search.stubborn;
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
        let pt = flow.story_playthrough(w);
        let steps = pt.steps.clone();
        let mut all = BTreeSet::new();
        every(
            &steps,
            &OrderNode {
                walk: flow.walk_for(&pt),
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

/// **The footprints against the replay, for tests.** Per realized branch,
/// every play state some legal order reaches (walked with no reduction), and
/// at each: every pair of legal steps the footprints call free of conflict is
/// taken in both orders — each must stay legal after the other, the two
/// orders must reach one play state, and each step must write the same lines
/// either way; and every step that is not legal must stay not legal after
/// every legal step outside its necessary enabling set. Returns the pairs and
/// the enabling sets checked, and every disagreement. Exponential in the
/// steps: a fixture's handful only.
#[doc(hidden)]
pub fn footprints_against_the_replay(c: &Campaign) -> (usize, usize, Vec<String>) {
    type Lines = Vec<(&'static str, String, HappeningVerb, Option<String>)>;
    fn lines_of(l: &[ChronicleLine]) -> Lines {
        l.iter()
            .map(|l| (l.kind, l.node.clone(), l.verb, l.subject.clone()))
            .collect()
    }
    fn take<'f, 'a, 'c>(
        n: &OrderNode<'f, 'a, 'c>,
        step: &PathStep,
    ) -> (OrderNode<'f, 'a, 'c>, Lines) {
        let mut walk = n.walk.clone();
        let j = walk.take(step);
        let mut ch = n.ch.clone();
        ch.push_step(&j);
        let mut tracker = n.tracker.clone();
        let l = ch.take_lines();
        for x in &l {
            let _ = tracker.read(x);
        }
        (
            OrderNode {
                walk,
                ch,
                tracker,
                order: Vec::new(),
            },
            lines_of(&l),
        )
    }
    let flow = Flow::new(c);
    let (mut pairs, mut sets, mut bad) = (0usize, 0usize, Vec::new());
    for b in enumerate(c) {
        let r = realize_one(c, &flow, b);
        let Some(w) = r.world else { continue };
        let m = OrderModel::new(c, &flow, w);
        let mut search = OrderSearch {
            steps: &m.steps,
            quiet: &m.quiet,
            fp: &m.fp,
            stubborn: 0,
            carried: &m.carried,
            read: &m.read,
            bound: usize::MAX,
            visited: BTreeSet::new(),
            flag_ids: BTreeMap::new(),
            line_ids: BTreeMap::new(),
            seen: BTreeSet::new(),
            found: Vec::new(),
            hit: false,
        };
        let mut stack = vec![OrderNode {
            walk: flow.walk_for(&m.pt),
            ch: Chronicler::new(c),
            tracker: ClashTracker::default(),
            order: Vec::new(),
        }];
        let id = &r.branch.id;
        while let Some(n) = stack.pop() {
            let key = search.state(&n);
            if !search.visited.insert(key) || n.walk.ended() {
                continue;
            }
            let legal: Vec<usize> = (0..m.steps.len())
                .filter(|&i| n.walk.legal(&m.steps[i]))
                .collect();
            for (x, &a) in legal.iter().enumerate() {
                for &b in &legal[x + 1..] {
                    if bit(&m.fp.conflict[a], b) || bit(&m.fp.conflict[b], a) {
                        continue;
                    }
                    pairs += 1;
                    let (sa, la) = take(&n, &m.steps[a]);
                    let (sb, lb) = take(&n, &m.steps[b]);
                    let (oa, ob) = (&m.steps[a].objective, &m.steps[b].objective);
                    if !sa.walk.legal(&m.steps[b]) || !sb.walk.legal(&m.steps[a]) {
                        bad.push(format!(
                            "{id}: `{oa}` and `{ob}` free, but one shuts the other"
                        ));
                        continue;
                    }
                    let (sab, lab) = take(&sa, &m.steps[b]);
                    let (sba, lba) = take(&sb, &m.steps[a]);
                    if search.state(&sab) != search.state(&sba) {
                        bad.push(format!(
                            "{id}: `{oa}` and `{ob}` free, but their orders part"
                        ));
                    }
                    if la != lba || lb != lab {
                        bad.push(format!("{id}: `{oa}` and `{ob}` free, but a line moves"));
                    }
                }
            }
            for (t, step) in m.steps.iter().enumerate() {
                if n.walk.done(&step.objective) || legal.contains(&t) {
                    continue;
                }
                let mut nes = vec![0u64; m.steps.len().div_ceil(64).max(1)];
                for &v in &m.fp.legal[t] {
                    if !m.fp.settled(v, &n.walk) {
                        for (a, w) in nes.iter_mut().zip(&m.fp.writers[v]) {
                            *a |= w;
                        }
                    }
                }
                for &a in &legal {
                    if bit(&nes, a) {
                        continue;
                    }
                    sets += 1;
                    let (sa, _) = take(&n, &m.steps[a]);
                    if sa.walk.legal(step) {
                        bad.push(format!(
                            "{id}: `{}` opens `{}` from outside its enabling set",
                            m.steps[a].objective, step.objective
                        ));
                    }
                }
            }
            for &a in &legal {
                stack.push(take(&n, &m.steps[a]).0);
            }
        }
    }
    bad.sort();
    bad.dedup();
    (pairs, sets, bad)
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OrderWalk {
    /// Steps on the branch's path.
    pub steps: usize,
    /// Of those, the quiet ones ([`quiet_steps`]).
    pub quiet: usize,
    /// Distinct play states the search walked.
    pub states: usize,
    /// Of those, the states where the stubborn set left a legal step out.
    pub stubborn: usize,
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
    fp: &'s Footprints,
    /// States where the stubborn set left out a legal step.
    stubborn: usize,
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
        let legal: Vec<usize> = (0..self.steps.len())
            .filter(|&i| n.walk.legal(&self.steps[i]))
            .collect();
        let walk = self.fp.stubborn(&n.walk, self.steps, &legal);
        self.stubborn += usize::from(walk.len() < legal.len());
        for i in walk {
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
///   objective's bundle, its quest's `on_complete` or the bundle of an entry
///   deed due in front of it carries a flag or numeric gate, and none fails an
///   objective (a failed objective's step is legal anywhere);
/// - it changes no gate another step reads: it sets no flag (by its option,
///   its bundle, its quest's `on_complete`, a choice or deed due in front of
///   it) that any `forbids_flags` anywhere, any effect's `requires_flags` or
///   the `forbids` of a deed of its world reads, directly or through an
///   ambient producer, and writes no datum any effect gate or deed gate
///   compares;
/// - it does not end the delve;
/// - none of its possible lines acts on a subject some step of the path can
///   leave in a carried state (`carried`).
fn quiet_steps(
    c: &Campaign,
    flow: &Flow<'_>,
    steps: &[PathStep],
    beats: &[Vec<(HappeningVerb, String)>],
    carried: &BTreeSet<String>,
    story: Option<&crate::compiler::flow::StoryCtx>,
) -> Vec<bool> {
    // Flags whose value some gate reads in a way that is not "more is more".
    let mut gates: BTreeSet<String> = BTreeSet::new();
    let mut read = data_gates_read(c);
    // A deed's own gate: a flag it forbids shuts the step it is due in front
    // of, and a datum it compares can open or shut it.
    for (_, a) in story.iter().flat_map(|ctx| &ctx.deeds) {
        let (forbids, data, _) = flow.act_gate_reads(a);
        gates.extend(forbids);
        read.extend(data);
    }
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
            // The choices and deeds due in front of the step travel with it.
            let due: Vec<&str> = story
                .map(|ctx| ctx.deeds_before(&step.objective).collect())
                .unwrap_or_default();
            for ch in story
                .into_iter()
                .flat_map(|ctx| ctx.choices_before(&step.objective))
            {
                sets.extend(ch.sets.iter().cloned());
            }
            for a in &due {
                sets.extend(flow.act_gate_reads(a).2);
            }
            let mut steady = true;
            let mut bundle = |e: &QuestEffect| {
                if !e.requires_flags().is_empty()
                    || !e.forbids_flags().is_empty()
                    || !e.requires_state().is_empty()
                    || matches!(
                        e.verb,
                        Verb::CampaignComplete { .. } | Verb::FailObjective { .. }
                    )
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
            for a in &due {
                each_effect(flow.act_effects(a), &mut bundle);
            }
            steady
                && !sets.iter().any(|f| gates.contains(f))
                && !beats.iter().any(|(v, s)| acts(*v) && carried.contains(s))
        })
        .collect()
}

/// One part of the play state a step can read or write: what the stubborn-set
/// reduction ([`Footprints`]) compares two steps by.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Var {
    /// A quest's progress: which step completes it, and so where its
    /// completion bundle fires and its own line plays.
    Quest(String),
    /// Whether a quest is running.
    Active(String),
    /// Whether an objective is done, failed or lapsed.
    Obj(String),
    /// Whether a flag is held.
    Flag(String),
    /// The value of a datum some gate compares.
    Data(String),
}

/// A set of step indices.
type Bits = Vec<u64>;

fn bit(b: &Bits, i: usize) -> bool {
    b[i / 64] & (1 << (i % 64)) != 0
}

fn set_bit(b: &mut Bits, i: usize) {
    b[i / 64] |= 1 << (i % 64);
}

/// **What each step of one branch's path touches** — the static half of the
/// stubborn-set reduction [`check_every_order`] runs (Valmari's stubborn sets;
/// Godefroid's persistent sets, "conflicting transitions" construction).
///
/// Two steps **conflict** when one can write what the other reads where a
/// write can change what it does or shut it (an effect gate, a `forbids` gate,
/// a compared datum, its own status), when both write one quest's progress or
/// one compared datum, when both have an acting line about one subject the
/// rules track, or when either can end the delve. A step's **legality inputs**
/// are everything [`Flow::step_legality_reads`] names. A write a step can make is every
/// flag, datum, objective status, quest progress and quest activity its own
/// option, the choices and deeds due in front of it, its objective's bundle or
/// expiry, and the completion bundles of the quests it can complete reach —
/// gated or not — closed over every ambient producer those flags (or a deed
/// it performs) can start. Flags are only ever set, quests only ever start
/// and objectives only ever settle, so a requirement already held cannot be
/// moved again, and a `requires` gate never shuts a step that is legal.
#[derive(Default)]
struct Footprints {
    vars: Vec<Var>,
    /// Each variable's index in `vars`.
    index: BTreeMap<Var, usize>,
    /// Per step, whether a choice or a deed is due in front of it.
    fronts: Vec<bool>,
    /// Per step, what the gates and the lines of the deeds due in front of it
    /// read: what can move what they do before the step is judged.
    deed_reads: Vec<Vec<usize>>,
    /// Per step, the objectives of the quest whose completion starts its own.
    opened_by: Vec<Vec<String>>,
    /// Per step, whether it can end the delve.
    ends: Vec<bool>,
    /// Per step, its plain `after` prerequisites.
    after: Vec<Vec<String>>,
    /// Per step, whether a deed due in front of it can fail it.
    fails_itself: Vec<bool>,
    /// Per step, the variables its legality reads.
    legal: Vec<Vec<usize>>,
    /// Per variable, the steps that can write it.
    writers: Vec<Bits>,
    /// Per step, the steps it conflicts with.
    conflict: Vec<Bits>,
}

impl Footprints {
    #[allow(clippy::too_many_lines)]
    fn new(
        c: &Campaign,
        flow: &Flow<'_>,
        steps: &[PathStep],
        carried: &BTreeSet<String>,
        read: &BTreeSet<String>,
        story: Option<&crate::compiler::flow::StoryCtx>,
    ) -> Self {
        let quests = &c.quests.content.quests;
        let quorum_members: BTreeSet<String> = crate::compiler::flow::quorums(c)
            .into_iter()
            .flat_map(|(_, q)| q.any.iter().map(|m| m.as_str().to_string()))
            .collect();
        let mut failable: BTreeSet<&str> = BTreeSet::new();
        for_each_campaign_effect(c, &mut |_, _, e| {
            if let Verb::FailObjective { objective } = &e.verb {
                failable.insert(objective.as_str());
            }
        });
        let words = steps.len().div_ceil(64).max(1);
        let mut ids: BTreeMap<Var, usize> = BTreeMap::new();
        let mut fp = Footprints::default();
        let mut id = |fp: &mut Footprints, v: Var| -> usize {
            *ids.entry(v.clone()).or_insert_with(|| {
                fp.vars.push(v);
                fp.writers.push(vec![0; words]);
                fp.vars.len() - 1
            })
        };
        let mut writes: Vec<BTreeSet<Var>> = Vec::new();
        let mut reads: Vec<BTreeSet<Var>> = Vec::new();
        let mut subjects: Vec<BTreeSet<String>> = Vec::new();
        let mut sets: Vec<BTreeSet<String>> = Vec::new();
        let mut ends: Vec<bool> = Vec::new();
        for step in steps {
            let (mut w, mut r) = (BTreeSet::new(), BTreeSet::new());
            // The compared data it writes other than by adding to them: two
            // additions to one datum land on one value in either order.
            let mut set: BTreeSet<String> = BTreeSet::new();
            let mut end = false;
            let due: Vec<&str> = story
                .map(|ctx| ctx.deeds_before(&step.objective).collect())
                .unwrap_or_default();
            // Whether this step can be the one that completes its quest: not
            // while an objective of the quest that settles only by its own
            // act waits on it, however far down its `after` chain.
            let own = quests.iter().find(|q| q.id.as_str() == step.quest);
            let last = own.is_none_or(|q| {
                let waits = |y: &delvewright_dsl::Objective| {
                    let mut stack: Vec<String> = y
                        .after_edges()
                        .into_iter()
                        .map(|a| a.as_str().to_string())
                        .collect();
                    let mut seen = BTreeSet::new();
                    while let Some(x) = stack.pop() {
                        if x == step.objective {
                            return true;
                        }
                        if seen.insert(x.clone())
                            && let Some(o) = q.objectives.iter().find(|o| o.id().as_str() == x)
                        {
                            stack.extend(
                                o.after_edges().into_iter().map(|a| a.as_str().to_string()),
                            );
                        }
                    }
                    false
                };
                !q.objectives.iter().any(|y| {
                    let id = y.id().as_str();
                    id != step.objective
                        && !failable.contains(id)
                        && !quorum_members.contains(id)
                        && waits(y)
                })
            });
            let due_effects: Vec<&[QuestEffect]> =
                due.iter().map(|a| flow.act_effects(a)).collect();
            let mut subj: BTreeSet<String> = possible_beats(c, step, &due_effects, last)
                .into_iter()
                .filter(|(v, s)| acts(*v) && carried.contains(s))
                .map(|(_, s)| s)
                .collect();
            // The quests this step can complete: its own, when it can be its
            // last, and any its completion starts whose every objective can
            // settle before it starts (a failed objective's step is legal
            // anywhere). Every step of a quest writes its progress: the
            // quest's own line plays at its first.
            w.insert(Var::Quest(step.quest.clone()));
            let mut completes: Vec<&str> = if last {
                vec![step.quest.as_str()]
            } else {
                Vec::new()
            };
            let mut k = 0;
            while k < completes.len() {
                let p = completes[k];
                k += 1;
                for o in quests {
                    if let delvewright_dsl::Trigger::QuestComplete { quest } = &o.trigger
                        && quest.as_str() == p
                    {
                        w.insert(Var::Active(o.id.as_str().to_string()));
                        if !completes.contains(&o.id.as_str())
                            && o.objectives
                                .iter()
                                .all(|x| failable.contains(x.id().as_str()))
                        {
                            completes.push(o.id.as_str());
                        }
                    }
                }
            }
            let mut effs: Vec<&[QuestEffect]> = Vec::new();
            for qid in &completes {
                w.insert(Var::Quest((*qid).to_string()));
                if let Some(q) = quests.iter().find(|q| q.id.as_str() == *qid) {
                    effs.push(&q.on_complete);
                    if *qid != step.quest {
                        let mut more = Vec::new();
                        fn lines(effs: &[QuestEffect], out: &mut Vec<(HappeningVerb, String)>) {
                            for e in effs {
                                if let Some(h) = &e.happening
                                    && let Some(s) = e.happening_subject()
                                {
                                    out.push((h.verb, s.id.to_string()));
                                }
                                for list in e.nested_effect_lists() {
                                    lines(list, out);
                                }
                            }
                        }
                        lines(&q.on_complete, &mut more);
                        subj.extend(
                            more.into_iter()
                                .filter(|(v, s)| acts(*v) && carried.contains(s))
                                .map(|(_, s)| s),
                        );
                    }
                }
            }
            if let Some(q) = quests.iter().find(|q| q.id.as_str() == step.quest) {
                if let Some(e) = q
                    .on_objective_complete
                    .get(&delvewright_dsl::ObjectiveId(step.objective.clone()))
                {
                    effs.push(e);
                }
                if let Some(o) = q
                    .objectives
                    .iter()
                    .find(|o| o.id().as_str() == step.objective)
                    && let Some(l) = o.limit()
                {
                    effs.push(l.on_expire());
                }
            }
            w.insert(Var::Obj(step.objective.clone()));
            r.insert(Var::Obj(step.objective.clone()));
            // A quorum that holds lapses its members (one quest's).
            for (_, qu) in crate::compiler::flow::quorums(c) {
                if qu.any.iter().any(|m| m.as_str() == step.objective) {
                    w.extend(qu.any.iter().map(|m| Var::Obj(m.as_str().to_string())));
                }
            }
            if let Some(n) = step.talk_option
                && let Some(npc) = talk_to_npc(c, &step.objective)
                && let Some((_, opt)) = option_at(c, npc, n)
            {
                for e in &opt.effects {
                    if let delvewright_dsl::DialogueEffect::SetFlag { flag } = e {
                        w.insert(Var::Flag(flag.as_str().to_string()));
                    }
                }
            }
            for ch in story
                .into_iter()
                .flat_map(|ctx| ctx.choices_before(&step.objective))
            {
                w.extend(ch.sets.iter().map(|f| Var::Flag(f.clone())));
            }
            for a in &due {
                if let Some(f) = flow.act_gate_reads(a).2 {
                    w.insert(Var::Flag(f));
                }
                effs.push(flow.act_effects(a));
            }
            for list in effs {
                each_effect(list, &mut |e| {
                    r.extend(
                        e.requires_flags()
                            .iter()
                            .map(|f| Var::Flag(f.as_str().to_string())),
                    );
                    r.extend(
                        e.forbids_flags()
                            .iter()
                            .map(|f| Var::Flag(f.as_str().to_string())),
                    );
                    r.extend(
                        e.requires_state()
                            .iter()
                            .map(|cmp| Var::Data(cmp.state.as_str().to_string())),
                    );
                    match &e.verb {
                        Verb::SetFlag { flag, .. } => {
                            w.insert(Var::Flag(flag.as_str().to_string()));
                        }
                        Verb::FailObjective { objective } => {
                            w.insert(Var::Obj(objective.as_str().to_string()));
                            r.insert(Var::Obj(objective.as_str().to_string()));
                        }
                        Verb::CampaignComplete { .. } => end = true,
                        _ => {}
                    }
                    if let Some((d, how)) = e.writes_state()
                        && read.contains(d.as_str())
                    {
                        w.insert(Var::Data(d.as_str().to_string()));
                        if !matches!(how, delvewright_dsl::StateWrite::Add(_)) {
                            set.insert(d.as_str().to_string());
                        }
                    }
                });
            }
            // What its legality reads that a write can shut: a flag it
            // forbids, a datum a gate compares.
            let sr = flow.step_legality_reads(step, story);
            r.extend(sr.forbids.iter().map(|f| Var::Flag(f.clone())));
            r.extend(sr.data.iter().map(|d| Var::Data(d.clone())));
            // Every ambient producer a flag it sets, or a deed it performs,
            // can start.
            loop {
                let mut changed = false;
                for (flag, requires, act) in flow.ambient_chains() {
                    let starts = requires.iter().any(|f| w.contains(&Var::Flag(f.clone())))
                        || act.is_some_and(|a| due.contains(&a));
                    if starts {
                        changed |= w.insert(Var::Flag(flag.to_string()));
                    }
                }
                if !changed {
                    break;
                }
            }
            let mut legal: Vec<usize> = Vec::new();
            legal.extend(
                sr.quests
                    .iter()
                    .map(|q| id(&mut fp, Var::Active(q.clone()))),
            );
            legal.extend(
                sr.objectives
                    .iter()
                    .map(|o| id(&mut fp, Var::Obj(o.clone()))),
            );
            legal.extend(
                sr.requires
                    .iter()
                    .chain(&sr.forbids)
                    .map(|f| id(&mut fp, Var::Flag(f.clone()))),
            );
            legal.extend(sr.data.iter().map(|d| id(&mut fp, Var::Data(d.clone()))));
            legal.sort_unstable();
            legal.dedup();
            fp.legal.push(legal);
            let fronts = story.is_some_and(|ctx| ctx.fronts(&step.objective));
            fp.fronts.push(fronts);
            // A deed fires before its step is judged, and a gate on one of its
            // lines can make it set a flag the step forbids: where a deed is
            // due, every flag its legality reads can shut it.
            if fronts {
                r.extend(sr.requires.iter().map(|f| Var::Flag(f.clone())));
            }
            let mut dr: Vec<usize> = Vec::new();
            for a in &due {
                let (requires, forbids, data) = flow.act_gate(a);
                for f in requires.into_iter().chain(forbids) {
                    dr.push(id(&mut fp, Var::Flag(f)));
                }
                for d in data {
                    dr.push(id(&mut fp, Var::Data(d)));
                }
                each_effect(flow.act_effects(a), &mut |e| {
                    for f in e.requires_flags().iter().chain(e.forbids_flags()) {
                        dr.push(id(&mut fp, Var::Flag(f.as_str().to_string())));
                    }
                    for cmp in e.requires_state() {
                        dr.push(id(&mut fp, Var::Data(cmp.state.as_str().to_string())));
                    }
                });
            }
            dr.sort_unstable();
            dr.dedup();
            fp.deed_reads.push(dr);
            let mut fails_itself = false;
            for a in &due {
                each_effect(flow.act_effects(a), &mut |e| {
                    if let Verb::FailObjective { objective } = &e.verb {
                        fails_itself |= objective.as_str() == step.objective;
                    }
                });
            }
            fp.fails_itself.push(fails_itself);
            fp.after.push(
                quests
                    .iter()
                    .flat_map(|q| &q.objectives)
                    .find(|o| o.id().as_str() == step.objective)
                    .map(|o| {
                        o.after_required()
                            .into_iter()
                            .map(|a| a.as_str().to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
            );
            fp.opened_by.push(
                quests
                    .iter()
                    .find(|q| q.id.as_str() == step.quest)
                    .and_then(|q| match &q.trigger {
                        delvewright_dsl::Trigger::QuestComplete { quest } => {
                            quests.iter().find(|p| p.id == *quest)
                        }
                        _ => None,
                    })
                    .map(|p| {
                        p.objectives
                            .iter()
                            .map(|o| o.id().as_str().to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
            );
            writes.push(w);
            reads.push(r);
            sets.push(set);
            subjects.push(subj);
            ends.push(end);
        }
        for (i, w) in writes.iter().enumerate() {
            for v in w {
                let k = id(&mut fp, v.clone());
                set_bit(&mut fp.writers[k], i);
            }
        }
        fp.index = ids;
        fp.conflict = vec![vec![0; words]; steps.len()];
        for a in 0..steps.len() {
            for b in (a + 1)..steps.len() {
                let ww = writes[a].iter().any(|v| match v {
                    Var::Quest(_) => writes[b].contains(v),
                    Var::Data(d) => {
                        writes[b].contains(v) && (sets[a].contains(d) || sets[b].contains(d))
                    }
                    _ => false,
                });
                let hit = ww
                    || !subjects[a].is_disjoint(&subjects[b])
                    || !writes[a].is_disjoint(&reads[b])
                    || !writes[b].is_disjoint(&reads[a]);
                if hit {
                    set_bit(&mut fp.conflict[a], b);
                    set_bit(&mut fp.conflict[b], a);
                }
            }
        }
        // A step that can end the delve shuts every other: nothing is walked
        // after it. A step it ends before does not threaten the step that
        // ends — an order with a step after the end is no order.
        for (a, end) in ends.iter().enumerate() {
            if *end {
                for b in 0..steps.len() {
                    if b != a {
                        set_bit(&mut fp.conflict[a], b);
                    }
                }
            }
        }
        fp.ends = ends;
        fp
    }

    /// Whether variable `v` can no longer change on `walk`: a quest already
    /// running, an objective already done, a flag already held.
    fn settled(&self, v: usize, walk: &crate::compiler::flow::Walk<'_, '_>) -> bool {
        match &self.vars[v] {
            Var::Active(q) => walk.active_quests().contains(q),
            Var::Obj(o) => walk.done(o),
            Var::Flag(f) => walk.flags().contains(f),
            Var::Quest(_) | Var::Data(_) => false,
        }
    }

    /// **A necessary enabling set** of step `t`, not legal at `walk`: steps one
    /// of which every order that makes `t` legal takes first. From the first
    /// conjunct of the replay's test that fails ([`Walk::blocked`]): a quest
    /// not running starts only once every objective of the quest that starts
    /// it is settled, so the writers of one of those not yet done; a missing
    /// prerequisite or flag, its writers; a quorum, the writers of its
    /// members; a flag it forbids is never cleared, so nothing; the reach of a
    /// dialogue option, the writers of every input of its legality not
    /// settled. Where a choice or deed is due in front of `t`, the writers of
    /// what those deeds read as well, since they fire before `t` is judged.
    /// Always the writers of `t`'s own status: a failed objective's step is
    /// legal anywhere.
    fn enablers(
        &self,
        t: usize,
        walk: &crate::compiler::flow::Walk<'_, '_>,
        step: &PathStep,
    ) -> Bits {
        use crate::compiler::flow::Blocked;
        let mut out = vec![0u64; self.conflict.len().div_ceil(64).max(1)];
        let mut or = |v: &Var| {
            if let Some(&k) = self.index.get(v) {
                for (a, w) in out.iter_mut().zip(&self.writers[k]) {
                    *a |= w;
                }
            }
        };
        or(&Var::Obj(step.objective.clone()));
        // A deed or a choice never starts a quest or settles an objective, so
        // unless a deed due in front of `t` can fail it, its quest not running
        // or a prerequisite not settled is a conjunct no deed can move.
        if !walk.failed(&step.objective) && !self.fails_itself[t] {
            if !walk.active_quests().contains(&step.quest) {
                if let Some(x) = self.opened_by[t].iter().find(|x| !walk.done(x)) {
                    or(&Var::Obj(x.clone()));
                }
                return out;
            }
            if let Some(a) = self.after[t].iter().find(|a| !walk.done(a)) {
                or(&Var::Obj(a.clone()));
                return out;
            }
        }
        let open = |vs: &[usize], or: &mut dyn FnMut(&Var)| {
            for &v in vs {
                if !self.settled(v, walk) {
                    or(&self.vars[v]);
                }
            }
        };
        if self.fronts[t] {
            open(&self.deed_reads[t], &mut or);
        }
        match walk.blocked(step) {
            None
            | Some(
                Blocked::NoQuest
                | Blocked::NoObjective
                | Blocked::NoOption(_)
                | Blocked::Forbids(_)
                | Blocked::Deed(_),
            ) => {}
            Some(Blocked::Inactive) => {
                if let Some(x) = self.opened_by[t].iter().find(|x| !walk.done(x)) {
                    or(&Var::Obj(x.clone()));
                }
            }
            Some(Blocked::After(a)) => or(&Var::Obj(a)),
            Some(Blocked::Quorum(ms)) => {
                for m in ms {
                    or(&Var::Obj(m));
                }
            }
            Some(Blocked::Requires(f)) => or(&Var::Flag(f)),
            Some(Blocked::Option(_)) => open(&self.legal[t], &mut or),
        }
        out
    }

    /// **A stubborn set at `walk`**, as the legal steps to walk: the smallest,
    /// over every legal seed, of the closure that adds, for a legal step, every
    /// step it conflicts with, and for a step that is not legal, every step
    /// that can write one of its legality inputs not yet settled (a necessary
    /// enabling set). Only steps not yet done are added. Every step outside
    /// the closure conflicts with no legal step inside it and cannot make a
    /// step inside it legal, so any order from here can take its first step of
    /// the closure first and keep the rest in their order — with the same
    /// state, the same lines and the same verdicts — and a first step of the
    /// closure is one of these.
    fn stubborn(
        &self,
        walk: &crate::compiler::flow::Walk<'_, '_>,
        steps: &[PathStep],
        legal: &[usize],
    ) -> Vec<usize> {
        let words = steps.len().div_ceil(64).max(1);
        let mut is_legal = vec![0u64; words];
        for &i in legal {
            set_bit(&mut is_legal, i);
        }
        let mut best: Option<Vec<usize>> = None;
        for &seed in legal {
            let mut in_t = vec![0u64; words];
            set_bit(&mut in_t, seed);
            let mut stack = vec![seed];
            while let Some(u) = stack.pop() {
                let mut add = vec![0u64; words];
                if bit(&is_legal, u) {
                    add.clone_from(&self.conflict[u]);
                } else {
                    add = self.enablers(u, walk, &steps[u]);
                }
                for (j, step) in steps.iter().enumerate() {
                    if bit(&add, j) && !bit(&in_t, j) && !walk.done(&step.objective) {
                        set_bit(&mut in_t, j);
                        stack.push(j);
                    }
                }
            }
            let p: Vec<usize> = legal.iter().copied().filter(|&i| bit(&in_t, i)).collect();
            if best.as_ref().is_none_or(|b| p.len() < b.len()) {
                let one = p.len() == 1;
                best = Some(p);
                if one {
                    break;
                }
            }
        }
        best.unwrap_or_default()
    }
}

/// Every `(verb, subject)` a step can put in the dated chronicle, whatever the
/// order: its quest's own line, its objective's, the option it takes, and every
/// effect its objective bundle, its quest's `on_complete` or the bundle of an
/// entry deed due in front of it (`deeds`) declares, gated or not — its
/// quest's `on_complete` only where `completes`. A superset: it decides which
/// subjects the rules track and which steps are quiet ([`quiet_steps`]), and
/// a superset can only make fewer steps quiet.
fn possible_beats(
    c: &Campaign,
    step: &PathStep,
    deeds: &[&[QuestEffect]],
    completes: bool,
) -> Vec<(HappeningVerb, String)> {
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
    if completes {
        effects(&q.on_complete, &mut out);
    }
    for d in deeds {
        effects(d, &mut out);
    }
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

/// One story world as the branch plan exports it (spec-0108 §4.3): the
/// harness walks it in a world of its own, exactly as it walked a branch.
#[derive(Clone, Debug)]
pub struct WorldRow {
    /// Its canonical index — `world/<k>`.
    pub k: usize,
    /// The world realized as a branch would be: its flags and its run.
    pub realized: RealizedBranch,
    /// The declared branches whose assignment its flags realize.
    pub branches: Vec<String>,
    /// The elective acts it performs and skips, by id.
    pub performs: Vec<String>,
    /// The elective acts it skips, by id.
    pub skips: Vec<String>,
    /// Per final-scene input: the class this world leaves it in.
    pub inputs: Vec<(String, String)>,
    /// Its entry deeds: `(act, the path step index it is performed in front of)`.
    pub entry_deeds: Vec<(String, usize)>,
    /// Why it exports no executable path, when it does not.
    pub no_path: Option<String>,
    /// Its dialogue selection: today's world index, and the flags each chosen
    /// alternative sets.
    pub selection: (usize, Vec<Vec<String>>),
}

impl WorldRow {
    /// The file slug: `world-<k>`.
    pub fn slug(&self) -> String {
        format!("world-{}", self.k)
    }

    /// How a message names this world: by the declared branches it realizes,
    /// with its id — `branch `branch/bolt` (world/1)` — or by its id alone.
    pub fn label(&self) -> String {
        if self.branches.is_empty() {
            format!("world/{}", self.k)
        } else {
            let names: Vec<String> = self.branches.iter().map(|b| format!("`{b}`")).collect();
            format!("branch {} (world/{})", names.join(", "), self.k)
        }
    }
}

/// Every story world, as a row of the branch plan (spec-0108 §4.3).
///
/// A world exports an executable path when its run is coherent, fires an
/// ending, and every entry deed is one the harness can perform; otherwise its
/// row names why not (§6.4).
pub fn world_rows(c: &Campaign) -> Vec<WorldRow> {
    let flow = Flow::new(c);
    let story = flow.story();
    let scenes = &story.scenes;
    let branches = enumerate(c);
    let ending_ids: BTreeMap<&str, &str> = scenes
        .endings
        .iter()
        .map(|e| (e.path.as_str(), e.id.as_str()))
        .collect();
    let undrivable: BTreeMap<&str, &str> = story
        .electives
        .iter()
        .filter_map(|e| e.undrivable.map(|r| (e.id.as_str(), r)))
        .collect();
    let mut out = Vec::new();
    for (k, w) in story.worlds.iter().enumerate() {
        let run = &story.runs[k];
        let pt = &run.playthrough;
        let journal = flow.journal(pt);
        let (chronicle, _) = chronicle_of(c, &journal, &skipped_acts(&flow, pt));
        let fired: Vec<String> = scenes
            .endings
            .iter()
            .filter(|e| run.fired.contains(&e.path))
            .filter_map(|e| ending_ids.get(e.path.as_str()).map(|s| s.to_string()))
            .collect();
        let selection_flags: BTreeSet<String> = flow
            .selection_flags(w.selection)
            .into_iter()
            .flatten()
            .collect();
        let entry = entry_choices(c, pt, &selection_flags);
        let realizes: Vec<String> = branches
            .iter()
            .filter(|b| {
                b.set.iter().all(|x| run.flags.contains(x))
                    && !b.unset.iter().any(|x| run.flags.contains(x))
            })
            .map(|b| b.id.clone())
            .collect();
        let forks: BTreeSet<String> = c
            .quest_plan
            .content
            .branch_points
            .iter()
            .flat_map(|bp| bp.forks_on.iter().map(|f| f.as_str().to_string()))
            .chain(scenes.scenes.iter().flat_map(|s| {
                s.inputs.iter().filter_map(|k| match k {
                    crate::compiler::ending::InputKey::Flag(f) => Some(f.clone()),
                    crate::compiler::ending::InputKey::Datum(_) => None,
                })
            }))
            .collect();
        let set: BTreeSet<String> = forks
            .iter()
            .filter(|f| run.flags.contains(*f))
            .cloned()
            .collect();
        let unset: BTreeSet<String> = forks.difference(&set).cloned().collect();
        let mut inputs = Vec::new();
        for scene in &scenes.scenes {
            let Some(snap) = run.scene_at.get(&scene.path) else {
                continue;
            };
            for key in &scene.inputs {
                let class = match key {
                    crate::compiler::ending::InputKey::Flag(f) => if snap.flags.contains(f) {
                        "set"
                    } else {
                        "unset"
                    }
                    .to_string(),
                    crate::compiler::ending::InputKey::Datum(d) => {
                        match snap.data.get(d).copied().flatten() {
                            Some(v) => scenes
                                .datum_classes(d)
                                .into_iter()
                                .find(|i| i.contains(v))
                                .map(|i| i.words())
                                .unwrap_or_else(|| v.to_string()),
                            None => "undatable".to_string(),
                        }
                    }
                };
                let row = (key.id().to_string(), class);
                if !inputs.contains(&row) {
                    inputs.push(row);
                }
            }
        }
        let deeds: Vec<(String, usize)> = pt
            .story
            .as_ref()
            .map(|ctx| ctx.deeds.iter().map(|(i, a)| (a.clone(), *i)).collect())
            .unwrap_or_default();
        // spec-0108 §6.5: an act no step kind drives is performed by mock — the
        // engine's own bundle function, asserted by the terms it changes. A
        // world still exports no path when its act has no such function, or
        // changes nothing the replay reads (a mock of it would assert nothing).
        let mocks = if deeds
            .iter()
            .any(|(a, _)| undrivable.contains_key(a.as_str()))
        {
            crate::compiler::plan::path_mocks(c, &flow, pt)
        } else {
            Vec::new()
        };
        let needs: Vec<String> = deeds
            .iter()
            .filter_map(|(a, _)| {
                let r = undrivable.get(a.as_str())?;
                let mock = mocks.iter().find_map(|(_, m)| match m {
                    crate::compiler::plan::Step::Mock { act, after, .. } if act == a => Some(after),
                    _ => None,
                });
                match mock {
                    Some(after) if !after.is_empty() => None,
                    Some(_) => Some(format!(
                        "`{a}`: {r}, and performing it changes no flag or datum the replay \
                         can name, so a mock of it would assert nothing"
                    )),
                    None => Some(format!(
                        "`{a}`: {r}, and its bundle has no engine function to mock it by"
                    )),
                }
            })
            .collect();
        let no_path = if let Some(why) = &run.incoherent {
            Some(format!(
                "unreachable: its run is not one a player can walk — {why}"
            ))
        } else if fired.is_empty() {
            Some("unreachable: its run fires no ending".to_string())
        } else if !needs.is_empty() {
            Some(format!(
                "its witness needs an act the harness can neither drive nor mock — {}",
                needs.join("; ")
            ))
        } else {
            None
        };
        let electives: Vec<String> = story.electives.iter().map(|e| e.id.clone()).collect();
        let realized = RealizedBranch {
            branch: EnumeratedBranch {
                id: format!("world/{k}"),
                slug: format!("world-{k}"),
                selection: BTreeMap::new(),
                set,
                unset,
                leads_to: Vec::new(),
                opens_at: Vec::new(),
            },
            world: Some(k),
            path: pt.steps.clone(),
            entry_choices: entry,
            chronicle,
            endings: fired,
            completed: run.completed.clone(),
        };
        out.push(WorldRow {
            k,
            realized,
            branches: realizes,
            performs: w.performs.iter().cloned().collect(),
            skips: electives
                .into_iter()
                .filter(|e| !w.performs.contains(e))
                .collect(),
            inputs,
            entry_deeds: deeds,
            no_path,
            selection: (w.selection, flow.selection_flags(w.selection)),
        });
    }
    out
}

/// Whether the campaign exports a branch plan at all: it declares a branch
/// point, or some elective act writes what an ending reads — the worlds then
/// differ in an ending's reading, and each is a run.
pub fn plans_worlds(c: &Campaign) -> bool {
    if !c.quest_plan.content.branch_points.is_empty() {
        return true;
    }
    let flow = Flow::new(c);
    // spec-0133 §5: every expiry is a world, whether or not an ending reads it.
    // An act that fails an objective is a world too.
    !flow.story().relevant.is_empty()
        || flow.electives().iter().any(|e| e.kind == "expiry")
        || !flow.failing_acts().is_empty()
}

/// The spec-0025/spec-0108 validation artifacts: `validation/branch-plan.json`,
/// one row per story world (§4.3), and one `validation/world-chronicle-<k>.md`
/// per world.
///
/// Validation metadata only — never part of the shipped datapack, listed in the
/// manifest exactly like `critical-path-waypoints.json`. Empty for a campaign
/// that declares no branch point and whose story worlds are its dialogue
/// selections, so nothing changes for anybody who has not opted in.
pub fn artifacts(c: &Campaign) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    if !plans_worlds(c) {
        return out;
    }
    let rows = world_rows(c);
    let realized: BTreeSet<String> = rows.iter().flat_map(|r| r.branches.clone()).collect();
    let unrealized: Vec<String> = enumerate(c)
        .into_iter()
        .map(|b| b.id)
        .filter(|id| !realized.contains(id))
        .collect();
    let plan = serde_json::json!({
        "version": c.quest_plan.dsl_version,
        "campaign_id": c.quest_plan.campaign_id.as_str(),
        "branches": rows.iter().map(|w| {
            let r = &w.realized;
            serde_json::json!({
                "id": r.branch.id,
                "branches": w.branches,
                "chronicle": format!("world-chronicle-{}.md", w.k),
                // The EXECUTABLE path the harness walks for this world, in the
                // `critical-path.json` contract (emitted by `emit::branch_paths`).
                // `null`, with the reason, for a world the harness cannot walk:
                // named, never silently absent.
                "path": if w.no_path.is_none() {
                    serde_json::Value::String(format!("world-path-{}.json", w.k))
                } else {
                    serde_json::Value::Null
                },
                "path_reason": w.no_path,
                "selection": {"index": w.selection.0, "alternatives": w.selection.1},
                "flags": {
                    "set": r.branch.set.iter().collect::<Vec<_>>(),
                    "unset": r.branch.unset.iter().collect::<Vec<_>>(),
                },
                "participation": {
                    "performs": w.performs,
                    "skips": w.skips,
                },
                "inputs": w.inputs.iter().map(|(k, c)| serde_json::json!({"key": k, "class": c})).collect::<Vec<_>>(),
                "reachable": w.no_path.as_deref().is_none_or(|r| !r.starts_with("unreachable")),
                "entry_choices": r.entry_choices.iter().map(|e| serde_json::json!({
                    "npc": e.npc,
                    "option": e.option,
                    // The chat line the option's dialog button runs. A dialog button is
                    // client-rendered and unclickable by a bot, so this is the
                    // player-legal actuation the harness sends — and what it asserts the
                    // world path really contains, so a run that never made the choice
                    // cannot pass as one.
                    "command": e.command,
                })).collect::<Vec<_>>(),
                // The elective acts the world performs on its path, each in front
                // of the step its run performed it at — what the harness asserts the
                // path really does, so a run cannot claim a deed world while walking
                // the extreme.
                "entry_deeds": w.entry_deeds.iter().map(|(act, at)| serde_json::json!({
                    "act": act,
                    "before_step": at,
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
            })
        }).collect::<Vec<_>>(),
        "branches_unrealized": unrealized,
    });
    let mut bytes = serde_json::to_vec_pretty(&plan).expect("branch plan serializes");
    bytes.push(b'\n');
    out.insert("validation/branch-plan.json".to_string(), bytes);
    for w in &rows {
        out.insert(
            format!("validation/world-chronicle-{}.md", w.k),
            chronicle_markdown(c, &w.realized).into_bytes(),
        );
    }
    out
}

/// The chronicle (流水账): one branch's storyline in compiled play order, from
/// first beat to ending, readable end to end against the campaign's DESIGN.md.
fn chronicle_markdown(c: &Campaign, r: &RealizedBranch) -> String {
    let mut s = String::new();
    let what = if r.branch.id.starts_with("world/") {
        "World"
    } else {
        "Branch"
    };
    s.push_str(&format!("# {what} chronicle — `{}`\n\n", r.branch.id));
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
