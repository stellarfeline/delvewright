//! The NPC scene ledger (spec-0020, `DW0460`–`DW0467`).
//!
//! ## Why the ledger exists
//!
//! An NPC used to carry **one dialogue tree for the whole campaign**. Owner
//! playtest, island round 12: after the climactic escape, `npc/perimedes` still
//! offered "Tell me what he is." and "Is there another way out?" — premise
//! questions absurd once the story has moved on. Per-node retirement was
//! *possible* (option-level flag gates) but authors did not reliably reach for
//! it, because nothing ever asked them to.
//!
//! The related defect (island round 8) is the same shape one layer down: two crew
//! NPCs stood forgotten in the stealth alcoves while the player escaped the cave.
//! The compiler held a provable effect history — spawn/move/despawn per NPC, beat
//! by beat — that nobody compared against the story's intent, because the intent
//! was never written down.
//!
//! The `cast` block writes the intent down. Every quest declares, for every live
//! NPC, **where** it is, **what it is doing**, and **what its right-click
//! offers**; this module compares the declaration against the effect history and
//! resolves it into the scenes the emitter swaps between.
//!
//! ## The four proofs (spec-0020 §2)
//!
//! 1. **Completeness** (`DW0460`) — every live NPC appears in every quest's
//!    `cast`. A missing entry names the NPC and the quest.
//! 2. **Placement consistency** (`DW0461`) — the declared `at` must equal the
//!    position [`crate::continuity`]'s replay actually produces when the quest
//!    opens. Declaring an anchor does not teleport anybody.
//! 3. **Dialogue gating** — the declaration *is* the gate. Enforced by
//!    construction rather than by a diagnostic: [`npc_casts`] resolves each
//!    quest's declaration into a scene and the emitter dispatches on it, so the
//!    sleeping giant's awake tree becomes unreachable *because the cast says his
//!    right-click is a sleep-murmur bark*.
//! 4. **Branch honesty** (`DW0462`) — where the history is branch-dependent, a
//!    single flat declaration cannot hold on every reachable branch; the quest
//!    must declare per-branch casts. No optimistic merging: this module inherits
//!    [`crate::continuity`]'s stance that an indeterminate history is reported,
//!    never guessed at.
//!
//! Supporting rules: `DW0463` (the forcing function — an on-stage placement must
//! say what the character is doing and what right-click offers), `DW0464`
//! (dangling refs), `DW0465` (the pre-0.7 deprecation window), `DW0466`
//! (`"unchanged"` with nothing to carry forward) and `DW0467` (the staleness
//! lint: an NPC whose dialogue never changes across the whole story).

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{
    Campaign, CastAbsence, CastDialogue, CastDialogueKeyword, CastEntry, CastPlacement, Diagnostic,
    is_v07,
};

use crate::continuity::{self, NpcWhere};

/// A live NPC is unaccounted for in a quest's `cast` ledger (proof 1).
pub const DW_CAST_UNACCOUNTED: &str = "DW0460";
/// A declared `at` contradicts the position the effect history produces (proof 2).
pub const DW_CAST_PLACEMENT: &str = "DW0461";
/// A branch-divergent NPC carries a single flat declaration (proof 4).
pub const DW_CAST_BRANCH: &str = "DW0462";
/// A cast placement omits the forcing-function fields, or declares them for a
/// body that is not in the world.
pub const DW_CAST_INCOMPLETE: &str = "DW0463";
/// A cast entry names something that does not exist (unknown NPC, a dialogue root
/// that is not a node of that NPC's tree, an empty bark pool).
pub const DW_CAST_DANGLING: &str = "DW0464";
/// A pre-0.7 campaign declares no cast ledger — the deprecation window (warning).
pub const DW_CAST_PRE_07: &str = "DW0465";
/// `"unchanged"` at an NPC's first appearance: nothing to carry forward.
pub const DW_CAST_UNCHANGED_FIRST: &str = "DW0466";
/// An NPC's dialogue never changes across the whole story (warning).
pub const DW_CAST_STALE: &str = "DW0467";

/// What an NPC's right-click does during one scene.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneAction {
    /// Open this node of the NPC's stage-6 tree (a dialogue root id).
    Root(String),
    /// Speak one line from this pool, cycling deterministically.
    Barks(Vec<String>),
    /// Nothing opens. The interaction is still recorded and consumed.
    Silent,
}

/// One resolved scene in an NPC's ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CastScene {
    /// The 1-based selector value the emitted dispatch uses.
    pub index: u32,
    /// What right-click does.
    pub action: SceneAction,
    /// The quest whose ledger first declared this scene (for artifact naming).
    pub declared_by: String,
}

/// One selector clause: "while this quest has begun (and this branch's flags
/// hold), that scene governs".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CastClause {
    /// The quest whose ledger declared it.
    pub quest: String,
    /// The scene index this clause selects.
    pub scene: u32,
    /// Branch gate: every listed flag must be set (per-branch casts).
    pub requires_flags: Vec<String>,
    /// Branch gate: no listed flag may be set.
    pub forbids_flags: Vec<String>,
}

/// One NPC's whole ledger, resolved into the scenes the emitter swaps between.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NpcCast {
    /// The distinct scenes, in declaration order; `scenes[i].index == i + 1`.
    pub scenes: Vec<CastScene>,
    /// The selector clauses in quest-DAG order, then declaration order within a
    /// quest. **Later clauses override earlier ones**, exactly as a later quest
    /// overrides an earlier one — so a per-branch entry lists its fallback first
    /// and its specific branches after. A quest that declares `"unchanged"`
    /// yields a clause pointing at the scene it carries forward, which is why the
    /// sugar emits no new artifact and still governs the right-click.
    pub by_quest: Vec<CastClause>,
}

/// Resolve every NPC's cast ledger into scenes, in quest-DAG order.
///
/// The single source of truth shared by the proofs below and the emitter: the
/// scene the compiler *checks* and the scene the datapack *shows* are computed
/// once, here. NPCs with no declaration anywhere are absent from the map, and
/// their emission is byte-identical to pre-0.7.
pub fn npc_casts(c: &Campaign) -> BTreeMap<String, NpcCast> {
    let mut out: BTreeMap<String, NpcCast> = BTreeMap::new();
    for qid in quest_dag_order(c) {
        let Some(q) = quest(c, &qid) else { continue };
        for (npc, entry) in &q.cast {
            // Every placement gets its own selector clause, so a per-branch entry
            // really does dispatch per branch. A declared absence
            // (`"offstage"`/`"dead"`) carries no dialogue and so yields no clause:
            // an NPC who is not in the world has no body to right-click.
            for p in entry.placements() {
                let Some(dialogue) = &p.dialogue else {
                    continue;
                };
                let cast = out.entry(npc.as_str().to_string()).or_default();
                let idx = match dialogue {
                    // `"unchanged"`: carry the previous scene forward. Emits
                    // nothing new — the dispatch keeps pointing at the same scene.
                    CastDialogue::Keyword(CastDialogueKeyword::Unchanged) => {
                        match cast.by_quest.last() {
                            Some(c) => c.scene,
                            None => continue, // DW0466 elsewhere; nothing to resolve
                        }
                    }
                    _ => {
                        let action = match dialogue {
                            CastDialogue::Keyword(CastDialogueKeyword::None) => SceneAction::Silent,
                            CastDialogue::Barks(b) => SceneAction::Barks(b.barks.clone()),
                            CastDialogue::Root(r) => SceneAction::Root(r.as_str().to_string()),
                            CastDialogue::Keyword(CastDialogueKeyword::Unchanged) => unreachable!(),
                        };
                        // Re-declaring the same scene reuses its index, so a
                        // repeated root id costs no extra artifact (it is still
                        // flagged as staleness by `DW0467`).
                        match cast.scenes.iter().find(|s| s.action == action) {
                            Some(s) => s.index,
                            None => {
                                let index = cast.scenes.len() as u32 + 1;
                                cast.scenes.push(CastScene {
                                    index,
                                    action,
                                    declared_by: qid.clone(),
                                });
                                index
                            }
                        }
                    }
                };
                cast.by_quest.push(CastClause {
                    quest: qid.clone(),
                    scene: idx,
                    requires_flags: p
                        .requires_flags
                        .iter()
                        .map(|f| f.as_str().to_string())
                        .collect(),
                    forbids_flags: p
                        .forbids_flags
                        .iter()
                        .map(|f| f.as_str().to_string())
                        .collect(),
                });
            }
        }
    }
    out
}

/// Whether a placement's branch gate holds under `flags`.
///
/// This is the emitted `cast_<npc>` selector's own gate, spelled once: the
/// clause it writes is `if <quest active> [if/unless each flag] run set <scene>`.
/// [`crate::branch`]'s `DW0483` and [`station`] both read it, so "which
/// placement governs this branch" has exactly one answer in the compiler.
pub fn selects(p: &CastPlacement, flags: &BTreeSet<String>) -> bool {
    p.requires_flags.iter().all(|f| flags.contains(f.as_str()))
        && !p.forbids_flags.iter().any(|f| flags.contains(f.as_str()))
}

/// Where the ledger stations one NPC at one beat, once a branch's flags are
/// known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Station<'a> {
    /// On stage at this anchor.
    At(&'a str),
    /// Declared out of the world (`"offstage"` / `"dead"`).
    Absent(CastAbsence),
}

/// The cast row that governs `npc` while the party plays `upto_quest`, under the
/// flags held at that moment — the ledger's answer to "where is this body?".
///
/// Resolution mirrors the emitted selector exactly (`cast_selector_fn`), because
/// a second model is how the static anchor and the ledger came to disagree in
/// the first place:
///
/// * clauses accumulate in [`quest_dag_order`] and **later declarations win** —
///   `dw.qa_<quest>` is set when a quest starts and never cleared, so an earlier
///   quest's row keeps governing until a later one replaces it;
/// * only quests that `begun` on this playthrough contribute (a branch never
///   activates the sibling branch's quests, so their clauses never fire);
/// * within one quest, the LAST placement whose gate holds wins — which is what
///   the emitted clause ladder does, and what `DW0483`'s message says it does.
///
/// `None` when no begun quest up to `upto_quest` declares this NPC at all: a
/// pre-0.7 campaign with no ledger, whose callers keep their old behavior.
pub fn station<'a>(
    c: &'a Campaign,
    npc: &str,
    upto_quest: &str,
    begun: &BTreeSet<String>,
    flags: &BTreeSet<String>,
) -> Option<Station<'a>> {
    let mut out = None;
    for qid in quest_dag_order(c) {
        if begun.contains(&qid)
            && let Some(q) = quest(c, &qid)
            && let Some((_, entry)) = q.cast.iter().find(|(k, _)| k.as_str() == npc)
        {
            if let Some(absence) = entry.absence() {
                out = Some(Station::Absent(absence));
            } else if let Some(p) = entry.placements().into_iter().rfind(|p| selects(p, flags)) {
                out = Some(match (p.at.anchor(), p.at.absence()) {
                    (Some(a), _) => Station::At(a.as_str()),
                    (None, Some(absence)) => Station::Absent(absence),
                    // `CastPlace` is an anchor or an absence; nothing else exists.
                    (None, None) => unreachable!("a cast place is an anchor or an absence"),
                });
            }
        }
        if qid == upto_quest {
            break;
        }
    }
    out
}

/// The placement that carries a quest's declaration for the whole-story lints
/// (`DW0466`/`DW0467`): the flat form, or the first per-branch placement. Those
/// lints ask "did this NPC's dialogue advance between beats?", which is a
/// question about the story, not about one branch — so they read one
/// representative placement rather than the cross-product of branches.
fn governing_placement(entry: &CastEntry) -> Option<&CastPlacement> {
    entry.placements().into_iter().next()
}

/// Run every cast-ledger proof over the campaign.
pub fn check_cast(c: &Campaign) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let v07 = is_v07(c.quests.dsl_version.as_str());
    let declares_any = c.quests.content.quests.iter().any(|q| !q.cast.is_empty());

    // The deprecation window (spec-0020 §4): a pre-0.7 campaign without a ledger
    // keeps building, once, with a warning naming the migration.
    if !v07 && !declares_any {
        diags.push(Diagnostic::warning(
            DW_CAST_PRE_07,
            "quests",
            "/dsl_version".to_string(),
            format!(
                "campaign is `dsl_version` `{}` and declares no `cast` ledger, so no quest says \
                 where its NPCs are, what they are doing, or what their right-click offers — the \
                 shape that let a crew member keep offering premise questions after the finale. \
                 Add a `cast` block to every quest and raise `dsl_version` to 0.7.0. This warning \
                 is the one-version deprecation window; the requirement hardens into an error \
                 after it",
                c.quests.dsl_version
            ),
        ));
        return diags;
    }
    if !declares_any {
        // A v0.7 campaign with no ledger at all: completeness reports it per
        // quest below, which names the NPCs concretely.
    }

    let timeline = continuity::replay(c);
    let order = quest_dag_order(c);
    let npc_ids: BTreeSet<String> = c
        .npcs
        .content
        .npcs
        .iter()
        .map(|n| n.id.as_str().to_string())
        .collect();

    for qid in &order {
        let Some(q) = quest(c, qid) else { continue };
        let qi = quest_index(c, qid);
        let here = timeline.at_quest_start.get(qid);

        // --- proof 1: completeness -------------------------------------------
        if v07 {
            for npc in &npc_ids {
                if q.cast.iter().any(|(k, _)| k.as_str() == npc.as_str()) {
                    continue;
                }
                let live = match here.and_then(|m| m.get(npc)) {
                    Some(NpcWhere::At(_)) | Some(NpcWhere::Indeterminate(_)) => true,
                    Some(NpcWhere::Offstage) | None => false,
                };
                if !live {
                    continue;
                }
                diags.push(Diagnostic::error(
                    DW_CAST_UNACCOUNTED,
                    "quests",
                    format!("/content/quests/{qi}/cast"),
                    format!(
                        "npc `{npc}` is live during quest `{qid}` but is unaccounted for in its \
                         `cast` — say where they are and what they are doing, or remove them from \
                         the world (`despawn-npc`) and declare them `\"offstage\"`/`\"dead\"`. An \
                         NPC nobody placed is how two crew members ended up standing forgotten in \
                         the alcoves while the player escaped"
                    ),
                ));
            }
        }

        for (npc, entry) in &q.cast {
            let npc_s = npc.as_str();
            let path = format!("/content/quests/{qi}/cast/{npc_s}");

            // --- dangling: the NPC itself ------------------------------------
            if !npc_ids.contains(npc_s) {
                diags.push(Diagnostic::error(
                    DW_CAST_DANGLING,
                    "quests",
                    path.clone(),
                    format!(
                        "quest `{qid}` casts `{npc_s}`, which is not a stage-2 npc — declare it in \
                         stage 2 or drop the entry"
                    ),
                ));
                continue;
            }

            let placements = entry.placements();

            // --- proof 4: branch honesty -------------------------------------
            let indeterminate = match here.and_then(|m| m.get(npc_s)) {
                Some(NpcWhere::Indeterminate(reason)) => Some(*reason),
                _ => None,
            };
            if let Some(reason) = indeterminate
                && placements.len() < 2
            {
                diags.push(Diagnostic::error(
                    DW_CAST_BRANCH,
                    "quests",
                    path.clone(),
                    format!(
                        "quest `{qid}` gives npc `{npc_s}` a single flat cast entry, but its \
                         position when this quest opens is branch-dependent: {reason}. One \
                         declaration cannot be true on every reachable branch, and merging them \
                         optimistically is how a ledger starts lying. Declare per-branch casts — a \
                         list of placements, each gated by the `requires_flags`/`forbids_flags` \
                         that select its branch"
                    ),
                ));
            }

            // --- per-placement checks ----------------------------------------
            for (i, p) in placements.iter().enumerate() {
                let ppath = if placements.len() > 1 {
                    format!("{path}/{i}")
                } else {
                    path.clone()
                };
                check_placement_shape(p, &ppath, npc_s, qid, &mut diags);
                check_placement_refs(c, p, &ppath, npc_s, &mut diags);
                if indeterminate.is_none() {
                    check_placement_position(p, &ppath, npc_s, qid, here, &mut diags);
                }
            }

            // The bare `"dead"`/`"offstage"` keyword form: still a position claim.
            if let Some(absence) = entry.absence()
                && indeterminate.is_none()
            {
                check_absence_position(absence, &path, npc_s, qid, here, &mut diags);
            }
        }
    }

    diags.extend(check_unchanged_and_staleness(c, &order));
    diags
}

/// `DW0463` — the forcing function. An on-stage placement must say what the
/// character is doing and what right-click offers; an off-stage one must not.
fn check_placement_shape(
    p: &CastPlacement,
    path: &str,
    npc: &str,
    qid: &str,
    diags: &mut Vec<Diagnostic>,
) {
    if p.at.anchor().is_some() {
        if p.doing.is_none() {
            diags.push(Diagnostic::error(
                DW_CAST_INCOMPLETE,
                "quests",
                path.to_string(),
                format!(
                    "npc `{npc}` stands on stage during quest `{qid}` but the cast entry says \
                     nothing about what they are `doing`. The field is prose the compiler never \
                     checks — it is required because you cannot fill it in without deciding the \
                     character's business in this beat, and stage 6 writes their lines against it"
                ),
            ));
        }
        if p.dialogue.is_none() {
            diags.push(Diagnostic::error(
                DW_CAST_INCOMPLETE,
                "quests",
                path.to_string(),
                format!(
                    "npc `{npc}` is clickable during quest `{qid}` but the cast entry does not say \
                     what their right-click offers. Declare `dialogue`: a root id (the branching \
                     tree), `{{\"barks\": [...]}}` (one inconsequential line, no consequences), \
                     `\"unchanged\"` (deliberately carry the previous scene forward), or \
                     `\"none\"` (genuinely no reaction — a last resort; if a body is clickable, \
                     the world should answer)"
                ),
            ));
        }
    } else if p.doing.is_some() || p.dialogue.is_some() {
        diags.push(Diagnostic::error(
            DW_CAST_INCOMPLETE,
            "quests",
            path.to_string(),
            format!(
                "npc `{npc}` is declared `\"{}\"` during quest `{qid}` — not in the world — yet \
                 the entry gives them `doing`/`dialogue`. A body that is not there has no business \
                 and answers no right-click: drop those fields, or put the character back on stage",
                p.at.token()
            ),
        ));
    }
}

/// `DW0464` — the refs a cast entry makes must exist.
fn check_placement_refs(
    c: &Campaign,
    p: &CastPlacement,
    path: &str,
    npc: &str,
    diags: &mut Vec<Diagnostic>,
) {
    match &p.dialogue {
        Some(CastDialogue::Root(root)) => {
            let known = c
                .dialogue
                .content
                .tree_for(npc)
                .is_some_and(|t| t.nodes.iter().any(|n| n.id.as_str() == root.as_str()));
            if !known {
                diags.push(Diagnostic::error(
                    DW_CAST_DANGLING,
                    "quests",
                    path.to_string(),
                    format!(
                        "cast dialogue root `{}` is not a node of npc `{npc}`'s stage-6 tree — the \
                         right-click would open nothing. Name a node this npc actually has",
                        root.as_str()
                    ),
                ));
            }
        }
        Some(CastDialogue::Barks(b)) if b.barks.is_empty() => {
            diags.push(Diagnostic::error(
                DW_CAST_DANGLING,
                "quests",
                path.to_string(),
                format!(
                    "npc `{npc}`'s bark pool is empty, so right-click would answer with silence — \
                     which is what a bark pool exists to prevent. Write at least one line, or \
                     declare `\"none\"` if the silence is the point"
                ),
            ));
        }
        _ => {}
    }
}

/// `DW0461` — a declared anchor must equal where the effect history leaves the
/// NPC when the quest opens.
fn check_placement_position(
    p: &CastPlacement,
    path: &str,
    npc: &str,
    qid: &str,
    here: Option<&BTreeMap<String, NpcWhere>>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(actual) = here.and_then(|m| m.get(npc)) else {
        return;
    };
    match (&p.at.anchor(), actual) {
        (Some(declared), NpcWhere::At(real)) if declared.as_str() != real => {
            diags.push(Diagnostic::error(
                DW_CAST_PLACEMENT,
                "quests",
                path.to_string(),
                format!(
                    "quest `{qid}` declares npc `{npc}` at `{}`, but the effect history leaves \
                     them at `{real}` when this quest opens — nothing walks them across. \
                     Declaring an anchor does not teleport anybody: add a `move-npc` to `{}` \
                     before this quest, or declare them where they actually stand (`{real}`)",
                    declared.as_str(),
                    declared.as_str()
                ),
            ));
        }
        (Some(declared), NpcWhere::Offstage) => {
            diags.push(Diagnostic::error(
                DW_CAST_PLACEMENT,
                "quests",
                path.to_string(),
                format!(
                    "quest `{qid}` declares npc `{npc}` at `{}`, but they are not in the world \
                     when this quest opens (never spawned, or despawned earlier). Bring them back \
                     with `spawn-npc`, or declare them `\"offstage\"`",
                    declared.as_str()
                ),
            ));
        }
        _ => {}
    }
}

/// `DW0461` for the absence forms: `"offstage"`/`"dead"` must match a despawn.
fn check_absence_position(
    absence: CastAbsence,
    path: &str,
    npc: &str,
    qid: &str,
    here: Option<&BTreeMap<String, NpcWhere>>,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(NpcWhere::At(real)) = here.and_then(|m| m.get(npc)) {
        diags.push(Diagnostic::error(
            DW_CAST_PLACEMENT,
            "quests",
            path.to_string(),
            format!(
                "quest `{qid}` declares npc `{npc}` `\"{}\"`, but their body is still standing at \
                 `{real}` when this quest opens — players can walk up and right-click a character \
                 the story has written out. Remove them with `despawn-npc` first, or declare where \
                 they actually stand",
                absence.token()
            ),
        ));
    }
}

/// `DW0466` (`"unchanged"` with nothing to carry) and `DW0467` (the staleness
/// lint: an NPC that appears in 2+ ledgers and never changes its dialogue).
fn check_unchanged_and_staleness(c: &Campaign, order: &[String]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    // npc -> the dialogue declarations it makes, in quest-DAG order.
    let mut seen: BTreeMap<&str, Vec<(&str, &CastDialogue)>> = BTreeMap::new();
    for qid in order {
        let Some(q) = quest(c, qid) else { continue };
        let qi = quest_index(c, qid);
        for (npc, entry) in &q.cast {
            let Some(p) = governing_placement(entry) else {
                continue;
            };
            let Some(d) = &p.dialogue else { continue };
            let log = seen.entry(npc.as_str()).or_default();
            if matches!(d, CastDialogue::Keyword(CastDialogueKeyword::Unchanged)) && log.is_empty()
            {
                diags.push(Diagnostic::error(
                    DW_CAST_UNCHANGED_FIRST,
                    "quests",
                    format!("/content/quests/{qi}/cast/{}", npc.as_str()),
                    format!(
                        "npc `{}` declares `\"unchanged\"` at its first appearance in quest \
                         `{qid}` — there is no previous scene to carry forward. `\"unchanged\"` \
                         states that keeping the current dialogue is a deliberate choice, so it \
                         needs something to keep: declare a real root id, a `barks` pool, or \
                         `\"none\"` here",
                        npc.as_str()
                    ),
                ));
            }
            log.push((qid.as_str(), d));
        }
    }

    // Staleness: 2+ appearances, and the *resolved* dialogue never changes.
    for (npc, log) in &seen {
        if log.len() < 2 {
            continue;
        }
        let mut resolved: Vec<&CastDialogue> = Vec::new();
        for (_, d) in log {
            match d {
                CastDialogue::Keyword(CastDialogueKeyword::Unchanged) => {
                    if let Some(prev) = resolved.last() {
                        resolved.push(prev);
                    }
                }
                other => resolved.push(other),
            }
        }
        let Some(first) = resolved.first() else {
            continue;
        };
        // A bark pool is background business by construction — the whole point is
        // that it does not advance — so only trees and silence can go stale.
        if matches!(first, CastDialogue::Barks(_)) {
            continue;
        }
        if resolved.len() < 2 || !resolved.iter().all(|d| d == first) {
            continue;
        }
        let what = match first {
            CastDialogue::Root(r) => format!("the same root `{}`", r.as_str()),
            _ => "no reaction at all".to_string(),
        };
        diags.push(Diagnostic::warning(
            DW_CAST_STALE,
            "quests",
            format!("/content/quests/{}/cast/{npc}", quest_index(c, log[0].0)),
            format!(
                "npc `{npc}` appears in {} quests' cast ledgers and offers {what} in every one — \
                 its right-click never learns that the story moved. That is the shape that left a \
                 crew member asking premise questions after the finale. Give it a scene that \
                 changes (a later root, retired options), or — if this really is a background \
                 character — a `barks` pool, which is allowed to stay the same because it never \
                 claims to advance anything",
                log.len()
            ),
        ));
    }
    diags
}

/// The stage-5 quest with this id.
fn quest<'a>(c: &'a Campaign, qid: &str) -> Option<&'a delvewright_dsl::Quest> {
    c.quests
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == qid)
}

/// The stage-5 declaration index of a quest (for diagnostic pointers).
fn quest_index(c: &Campaign, qid: &str) -> usize {
    c.quests
        .content
        .quests
        .iter()
        .position(|q| q.id.as_str() == qid)
        .unwrap_or(0)
}

/// Stage-5 quest ids in the same DAG linearization [`crate::continuity`] replays,
/// so "the previous appearance" means the same thing to the ledger, the proofs
/// and the emitter.
pub fn quest_dag_order(c: &Campaign) -> Vec<String> {
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
                let pos = ready.partition_point(|&r| r < dep);
                ready.insert(pos, dep);
            }
        }
    }
    let mut out: Vec<String> = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for i in order {
        let id = ids[i];
        if c.quests.content.quests.iter().any(|q| q.id.as_str() == id) {
            out.push(id.to_string());
            seen.insert(id);
        }
    }
    for q in &c.quests.content.quests {
        if !seen.contains(q.id.as_str()) {
            out.push(q.id.as_str().to_string());
        }
    }
    out
}
