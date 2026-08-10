//! **The one enumeration of the campaign's effect roots.**
//!
//! An *effect root* is a `Vec<QuestEffect>` that emission can lower. There are
//! seven, they hang off two different stage documents, and nothing about the
//! shape of the DSL makes them findable by inspection — which is why every walk
//! that needed "every effect" was, historically, written by someone enumerating
//! the roots they happened to know about.
//!
//! Six separate investigations each found one such walk and fixed it in place
//! (`plan::collect_gate_events`, `l10n::each_string`, `timeline::walk_campaign` /
//! `nav::all_effects`, `flow::read_flags`, `emit::check_effect_anchors`,
//! `emit::declared_flags`). A sweep after the sixth found **thirteen more**
//! walkers still enumerating three or four of the five. None of them was red:
//! a walk that visits four of five roots produces correct-looking output over
//! any campaign that happens not to use the fifth, and it stays green until a
//! campaign uses it.
//!
//! Fixing thirteen walkers by hand fixes thirteen walkers. This module exists so
//! that the *next* root — and there will be one; the fifth was added once
//! already — is added in **one** place and every consumer inherits it:
//!
//! * [`for_each_effect_root`] is the single immutable enumeration. Every
//!   campaign-wide effect walk in the workspace is defined in terms of it or of
//!   [`for_each_campaign_effect`], which is defined in terms of it.
//! * [`for_each_effect_root_mut`] is its mutable mirror, generated from the
//!   **same macro body** ([`effect_root_walk`]) rather than written out a second
//!   time, so the two cannot drift: the root list exists once as tokens.
//! * [`EffectRootKind`] names the roots. `EffectRootKind::ALL` is the closed set;
//!   the walk asserts it visited every member on every call ([`RootBinding`]),
//!   so a root that stops being enumerated is a panic in every build rather than
//!   a quietly narrower answer.
//! * Consumers that need to know *which* root a bundle is match on
//!   [`EffectRootOwner`]. Adding a variant there is a rustc error at every such
//!   site, so a new root cannot be silently mis-classified either.
//!
//! Roots 6 and 7 were added by spec-0031, and they are worth reading as a pair
//! because they are the two ways this defect class recurs after the enumeration
//! exists:
//!
//! * **R6 `shortcuts[].on_unlock` was already a root and nobody had noticed.**
//!   It is a `Vec<QuestEffect>` hanging off a stage-5 struct, structurally
//!   identical in kind to `traps[].payload` (which is R4), and emission really
//!   lowers it (`emit::emit_shortcut_functions`). It was simply never listed —
//!   so every proof, every l10n pass and every diagnostic written for "the
//!   general path" silently did not cover it: a `narrate` inside it was never
//!   inventoried, a `set-flag` inside it was invisible to the flag model, and a
//!   `sequence` inside it would have emitted a `function` call to a function
//!   nothing generated. Zero campaigns happened to use it, which is the only
//!   reason it never shipped as a bug. The sixth blind spot in the family that
//!   `#301`/`#302`/`#321` each closed one instance of.
//! * **R7 `on_death` is new surface that starts inside the enumeration.** The
//!   whole point of adding it as a root, rather than as a hook on the checkpoint
//!   machinery that detects death, is that "the purse is dropped on death" then
//!   stops being an engine feature and becomes ordinary content in a general
//!   mechanism.
//!
//! What this module deliberately does **not** try to be is a guard against a
//! fourteenth hand-rolled walk being written tomorrow. Nothing in the type system
//! can stop someone iterating `campaign.quests.content.quests` directly; that
//! half of the obligation is `tools/check-effect-roots.py`, which fails CI when a
//! source file reaches for two or more root fields outside this module.
//!
//! Determinism (ADR-0006): iteration is over `BTreeMap` keys and slices, in a
//! fixed order that is part of this module's contract — see
//! [`for_each_effect_root`].

use crate::envelope::Campaign;
use crate::stages::{EnvTrigger, Quest, QuestEffect, Shortcut, Trap};

/// The local part of a type-prefixed id (`npc/keeper` → `keeper`), the segment
/// every l10n key is built from. Duplicated from `l10n::local` deliberately: this
/// module is below `l10n` and the key scheme is part of a root's identity.
fn local(id: &str) -> &str {
    id.split_once('/').map(|(_, r)| r).unwrap_or(id)
}

/// Which of the campaign's effect roots a bundle is.
///
/// `ALL` is the closed set. Adding a variant is a rustc error in
/// [`EffectRootOwner::kind`] and in every consumer that matches on an owner, and
/// makes `ALL`'s length wrong until it is listed — so a new root cannot be added
/// without visiting the walk.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum EffectRootKind {
    /// A quest's `on_objective_complete[<objective>]` bundle.
    ObjectiveComplete,
    /// A quest's `on_complete` bundle.
    QuestComplete,
    /// An environment trigger's `effects` bundle.
    Trigger,
    /// A trap's spec-0022 `payload` bundle.
    TrapPayload,
    /// A dialogue option's `set-checkpoint` `on_respawn` bundle — a plain
    /// `Vec<QuestEffect>` hanging off the **dialogue** stage. `DialogueEffect`
    /// carries no gate, movement or actor verb of its own, which is the reasoning
    /// that made every older walk stop at the quests stage; the bundle nested
    /// inside one is quest-effect vocabulary all the same, and it is lowered
    /// (into `cp_on_respawn_<i>`).
    DialogueRespawn,
    /// A `shortcuts[].on_unlock` bundle (spec-0016 §2) — the beat that plays as
    /// the bar lifts. Lowered by `emit::emit_shortcut_functions` into
    /// `shortcut_open_<id>`, and unenumerated until spec-0031: the sixth blind
    /// spot, structurally the same shape as R4.
    ShortcutUnlock,
    /// The campaign's `on_death` bundle (DSL v0.10, spec-0031) — the effects that
    /// run at the moment a player dies, for that player. One per campaign, and
    /// visited only when non-empty, so `unbound_roots` tells the truth about a
    /// campaign that declares no death beat.
    OnDeath,
}

impl EffectRootKind {
    /// Every root, in enumeration order. Not the *visit* order — see
    /// [`for_each_effect_root`], which interleaves R1/R2 per quest.
    pub const ALL: [EffectRootKind; 7] = [
        EffectRootKind::ObjectiveComplete,
        EffectRootKind::QuestComplete,
        EffectRootKind::Trigger,
        EffectRootKind::TrapPayload,
        EffectRootKind::DialogueRespawn,
        EffectRootKind::ShortcutUnlock,
        EffectRootKind::OnDeath,
    ];

    /// How many roots there are. The binding ledger reports coverage against this.
    pub const COUNT: usize = Self::ALL.len();

    /// The stage document this root lives in (`quests` or `dialogue`).
    pub fn stage(self) -> &'static str {
        match self {
            EffectRootKind::ObjectiveComplete
            | EffectRootKind::QuestComplete
            | EffectRootKind::Trigger
            | EffectRootKind::TrapPayload
            | EffectRootKind::ShortcutUnlock
            | EffectRootKind::OnDeath => "quests",
            EffectRootKind::DialogueRespawn => "dialogue",
        }
    }

    /// A short human label, used by the binding ledger and by diagnostics that
    /// report which roots a proof examined.
    pub fn label(self) -> &'static str {
        match self {
            EffectRootKind::ObjectiveComplete => "quest on_objective_complete",
            EffectRootKind::QuestComplete => "quest on_complete",
            EffectRootKind::Trigger => "trigger effects",
            EffectRootKind::TrapPayload => "trap payload",
            EffectRootKind::DialogueRespawn => "dialogue set-checkpoint on_respawn",
            EffectRootKind::ShortcutUnlock => "shortcut on_unlock",
            EffectRootKind::OnDeath => "campaign on_death",
        }
    }
}

/// What a root hangs off, with the owning object attached.
///
/// This is what a consumer matches on when it needs to reason about *when* a
/// bundle fires (the completability model) or *who* gates it (a trigger's or
/// trap's `requires_flags`). Because the match is exhaustive at every such site,
/// an eighth root is a compile error everywhere the answer would have to change.
#[derive(Clone, Copy)]
pub enum EffectRootOwner<'a> {
    /// A quest's `on_objective_complete[<objective>]` — fires at that objective's
    /// `critical_path` step. Forced: completing the objective is the mainline.
    ObjectiveComplete {
        /// The owning quest.
        quest: &'a Quest,
        /// The objective whose completion fires the bundle.
        objective: &'a str,
    },
    /// A quest's `on_complete` — fires at the quest's completion step. Forced.
    QuestComplete {
        /// The owning quest.
        quest: &'a Quest,
    },
    /// An environment `triggers[].effects` — proximity/interaction-fired, so it
    /// has no step of its own. Carries the trigger, whose `requires_flags` gate
    /// the whole bundle.
    Trigger(&'a EnvTrigger),
    /// A `traps[].payload` (spec-0022) — proximity/interaction-fired exactly like
    /// a trigger, and **optional**: the party may never trip it. Carries the trap,
    /// whose `requires_flags` gate the whole payload.
    TrapPayload(&'a Trap),
    /// A dialogue option's `set-checkpoint` `on_respawn` bundle — re-run on death
    /// while that checkpoint is active, so it is optional too (nobody is forced to
    /// die). Carries no owning object: the npc, node and option are all named in
    /// the site's `path`, and no consumer needs to reach the tree itself.
    DialogueRespawn,
    /// A `shortcuts[].on_unlock` (spec-0016 §2) — fired once, by the far-side
    /// interaction, and **optional**: `Plan::build` registers every shortcut gate
    /// as sealed at step 0 precisely so the delve is proven completable with no
    /// shortcut ever taken. Carries the shortcut; it declares no flag gate of its
    /// own, so the whole bundle is ungated.
    ShortcutUnlock(&'a Shortcut),
    /// The campaign's `on_death` (spec-0031) — fired at the moment a player dies,
    /// so it has no step of its own and is **optional** in the strongest sense:
    /// nobody is forced to die. Carries no owning object; there is exactly one
    /// per campaign and its path is `/content/on_death`.
    OnDeath,
}

impl<'a> EffectRootOwner<'a> {
    /// Which root this is. The one place a root's owner is mapped to its kind.
    pub fn kind(&self) -> EffectRootKind {
        match self {
            EffectRootOwner::ObjectiveComplete { .. } => EffectRootKind::ObjectiveComplete,
            EffectRootOwner::QuestComplete { .. } => EffectRootKind::QuestComplete,
            EffectRootOwner::Trigger(_) => EffectRootKind::Trigger,
            EffectRootOwner::TrapPayload(_) => EffectRootKind::TrapPayload,
            EffectRootOwner::DialogueRespawn => EffectRootKind::DialogueRespawn,
            EffectRootOwner::ShortcutUnlock(_) => EffectRootKind::ShortcutUnlock,
            EffectRootOwner::OnDeath => EffectRootKind::OnDeath,
        }
    }

    /// The quest this root belongs to, if it has a DAG position at all.
    pub fn quest(&self) -> Option<&'a Quest> {
        match self {
            EffectRootOwner::ObjectiveComplete { quest, .. }
            | EffectRootOwner::QuestComplete { quest } => Some(quest),
            EffectRootOwner::Trigger(_)
            | EffectRootOwner::TrapPayload(_)
            | EffectRootOwner::DialogueRespawn
            | EffectRootOwner::ShortcutUnlock(_)
            | EffectRootOwner::OnDeath => None,
        }
    }
}

/// One effect root: which it is, where it is, and what its l10n keys hang off.
///
/// `path` points at the **list**; an element's pointer is `path` + `/<index>`.
/// `key` is likewise the list's keybase; an element's key is `key` + `.<index>`.
pub struct EffectRootSite<'a> {
    /// What this root hangs off, with the owning object.
    pub owner: EffectRootOwner<'a>,
    /// The stage document the list lives in (`quests` or `dialogue`).
    pub stage: &'static str,
    /// JSON pointer to the list within that document.
    pub path: String,
    /// The list's l10n key prefix.
    pub key: String,
}

impl EffectRootSite<'_> {
    /// Which root this site is.
    pub fn kind(&self) -> EffectRootKind {
        self.owner.kind()
    }
}

/// What a walk over the effect roots actually examined.
///
/// CLAUDE.md: *a green gate that binds to nothing is vacuous, not a pass*. A proof
/// over "every effect" is only as good as the roots it reached and the bundles it
/// found there, and neither number is visible from the proof's own output. This
/// is that ledger, filled in by [`for_each_effect_root`] on every call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootBinding {
    /// How many of [`EffectRootKind::COUNT`] roots the walk enumerated. Always
    /// `COUNT` for a walk that ran — a smaller number means a root stopped being
    /// enumerated, which the walk itself asserts against.
    pub roots_enumerated: usize,
    /// Per-root: how many bundles the campaign actually has there. A zero is not
    /// a failure — a campaign with no traps has no `traps[].payload` — but it is
    /// the reason a proof over that root binds to nothing, and it is reported
    /// rather than left for a reader to infer.
    pub sites: [(EffectRootKind, usize); EffectRootKind::COUNT],
    /// Total top-level effects across every root.
    pub effects: usize,
}

impl RootBinding {
    /// The roots this campaign has no bundles at — where any proof over the
    /// effect surface is necessarily unbound.
    pub fn unbound_roots(&self) -> Vec<EffectRootKind> {
        self.sites
            .iter()
            .filter(|(_, n)| *n == 0)
            .map(|(k, _)| *k)
            .collect()
    }

    /// A one-line, deterministic rendering for a report or a `--json` field.
    pub fn summary(&self) -> String {
        let per: Vec<String> = self
            .sites
            .iter()
            .map(|(k, n)| format!("{}={n}", k.label()))
            .collect();
        format!(
            "roots {}/{}, bundles {}, effects {} [{}]",
            self.roots_enumerated,
            EffectRootKind::COUNT,
            self.sites.iter().map(|(_, n)| n).sum::<usize>(),
            self.effects,
            per.join(", ")
        )
    }
}

/// **The root list, written once, as tokens.**
///
/// Expanded twice — by [`for_each_effect_root`] with `iter`/`as_slice` and by
/// [`for_each_effect_root_mut`] with `iter_mut`/`as_mut_slice`. There is no second
/// copy of "which lists are roots" anywhere in the workspace, so adding a root is
/// one edit here and every consumer of either walk inherits it (roots 6 and 7 were
/// added by spec-0031 and this claim is what made it a small change). That is
/// the whole point of this module: the previous arrangement had the root list
/// written out four times (twice in `l10n`, once in `plan`, once in `stages`) and
/// approximated a further thirteen times by walkers that enumerated three or four
/// of the five.
///
/// `$visit` is called as `$visit((kind, owner, objective), path, key, list)`. The
/// per-root owner expressions are parameters because the mutable expansion cannot
/// produce them: it cannot hand out `&Quest` while holding `&mut [QuestEffect]`
/// from the same quest. That asymmetry is confined to what is *attached* to a
/// visit — never to which roots are visited, which is what this body fixes.
macro_rules! effect_root_walk {
    (
        campaign: $c:expr,
        iter: $iter:ident,
        slice: $slice:ident,
        respawn: $respawn:ident,
        note: $note:expr,
        visit: $visit:expr,
        quest_owner: |$q:ident| $ownq:expr,
        trigger_owner: |$t:ident| $ownt:expr,
        trap_owner: |$p:ident| $ownp:expr,
        dialogue_owner: $ownd:expr,
        shortcut_owner: |$s:ident| $owns:expr,
        death_owner: $ownx:expr,
    ) => {{
        #[allow(unused_mut)]
        let mut visit = $visit;
        // Fired once per root, before its loop, whether or not this campaign has a
        // single bundle there. That is the distinction the binding ledger exists to
        // make: "this walk enumerated the root" and "this campaign uses the root"
        // are different facts, and a proof that conflates them reports a vacuous
        // green as a pass (CLAUDE.md).
        #[allow(unused_mut)]
        let mut note = $note;
        // R1 `on_objective_complete` and R2 `on_complete`, interleaved per quest.
        // This order is contractual: it is the order emission writes bundles in and
        // the order the l10n inventory keys them in, so a campaign that predates a
        // later root produces byte-identical output.
        note(EffectRootKind::ObjectiveComplete);
        note(EffectRootKind::QuestComplete);
        for (qi, $q) in $c.quests.content.quests.$iter().enumerate() {
            let ql = local($q.id.as_str()).to_string();
            let owner = $ownq;
            for (oid, effs) in $q.on_objective_complete.$iter() {
                let ol = local(oid.as_str()).to_string();
                visit(
                    (EffectRootKind::ObjectiveComplete, owner, Some(oid.as_str())),
                    format!(
                        "/content/quests/{qi}/on_objective_complete/{}",
                        oid.as_str()
                    ),
                    format!("fx.{ql}.oc.{ol}"),
                    effs.$slice(),
                );
            }
            visit(
                (EffectRootKind::QuestComplete, owner, None),
                format!("/content/quests/{qi}/on_complete"),
                format!("fx.{ql}.done"),
                $q.on_complete.$slice(),
            );
        }
        // R3 `triggers[].effects`.
        note(EffectRootKind::Trigger);
        for (ti, $t) in $c.quests.content.triggers.$iter().enumerate() {
            let tl = local($t.id.as_str()).to_string();
            let owner = $ownt;
            visit(
                (EffectRootKind::Trigger, owner, None),
                format!("/content/triggers/{ti}/effects"),
                format!("fx.trig.{tl}"),
                $t.effects.$slice(),
            );
        }
        // R4 `traps[].payload` (spec-0022 — a payload is an effect root).
        note(EffectRootKind::TrapPayload);
        for (pi, $p) in $c.quests.content.traps.$iter().enumerate() {
            let pl = local($p.id.as_str()).to_string();
            let owner = $ownp;
            visit(
                (EffectRootKind::TrapPayload, owner, None),
                format!("/content/traps/{pi}/payload"),
                format!("fx.trap.{pl}"),
                $p.payload.$slice(),
            );
        }
        // R5 a dialogue option's `set-checkpoint` `on_respawn` bundle — the root
        // that hangs off a different stage document, and the one every walk written
        // from "effects live in the quests stage" missed.
        note(EffectRootKind::DialogueRespawn);
        for (di, tree) in $c.dialogue.content.dialogues.$iter().enumerate() {
            let np = local(tree.npc.as_str()).to_string();
            let owner = $ownd;
            for (ni, node) in tree.nodes.$iter().enumerate() {
                let nd = local(node.id.as_str()).to_string();
                for (oi, opt) in node.options.$iter().enumerate() {
                    for (ei, de) in opt.effects.$iter().enumerate() {
                        let Some(on_respawn) = de.$respawn() else {
                            continue;
                        };
                        visit(
                            (EffectRootKind::DialogueRespawn, owner, None),
                            format!(
                                "/content/dialogues/{di}/nodes/{ni}/options/{oi}/effects/{ei}/on_respawn"
                            ),
                            format!("fx.dlg.{np}.{nd}.{oi}.{ei}.respawn"),
                            on_respawn,
                        );
                    }
                }
            }
        }
        // R6 `shortcuts[].on_unlock` (spec-0016 §2) — an effect bundle emission
        // has always lowered and no enumeration knew about, closed by spec-0031.
        note(EffectRootKind::ShortcutUnlock);
        for (si, $s) in $c.quests.content.shortcuts.$iter().enumerate() {
            let sl = local($s.id.as_str()).to_string();
            let owner = $owns;
            visit(
                (EffectRootKind::ShortcutUnlock, owner, None),
                format!("/content/shortcuts/{si}/on_unlock"),
                format!("fx.sc.{sl}"),
                $s.on_unlock.$slice(),
            );
        }
        // R7 the campaign's `on_death` (DSL v0.10, spec-0031) — one bundle, no
        // owning object, visited only when the campaign declares one. An empty
        // list is NOT visited: `RootBinding` must be able to say "this campaign
        // has no death beat", which a site count that is always 1 could not.
        note(EffectRootKind::OnDeath);
        {
            let owner = $ownx;
            let on_death = $c.quests.content.on_death.$slice();
            if !on_death.is_empty() {
                visit(
                    (EffectRootKind::OnDeath, owner, None),
                    "/content/on_death".to_string(),
                    "fx.death".to_string(),
                    on_death,
                );
            }
        }
    }};
}

/// The owning object as the macro yields it, before it is paired with the
/// objective id that only R1 has. Internal to [`for_each_effect_root`].
#[derive(Clone, Copy)]
enum RawOwner<'a> {
    Quest(&'a Quest),
    Trigger(&'a EnvTrigger),
    Trap(&'a Trap),
    Dialogue,
    Shortcut(&'a Shortcut),
    Death,
}

impl<'a> RawOwner<'a> {
    fn attach(self, kind: EffectRootKind, objective: Option<&'a str>) -> EffectRootOwner<'a> {
        match (self, kind) {
            (RawOwner::Quest(quest), EffectRootKind::ObjectiveComplete) => {
                EffectRootOwner::ObjectiveComplete {
                    quest,
                    objective: objective
                        .expect("an on_objective_complete root always names its objective"),
                }
            }
            (RawOwner::Quest(quest), EffectRootKind::QuestComplete) => {
                EffectRootOwner::QuestComplete { quest }
            }
            (RawOwner::Trigger(t), EffectRootKind::Trigger) => EffectRootOwner::Trigger(t),
            (RawOwner::Trap(p), EffectRootKind::TrapPayload) => EffectRootOwner::TrapPayload(p),
            (RawOwner::Dialogue, EffectRootKind::DialogueRespawn) => {
                EffectRootOwner::DialogueRespawn
            }
            (RawOwner::Shortcut(s), EffectRootKind::ShortcutUnlock) => {
                EffectRootOwner::ShortcutUnlock(s)
            }
            (RawOwner::Death, EffectRootKind::OnDeath) => EffectRootOwner::OnDeath,
            (owner, kind) => unreachable!(
                "effect root {kind:?} was handed an owner of the wrong shape ({})",
                match owner {
                    RawOwner::Quest(_) => "quest",
                    RawOwner::Trigger(_) => "trigger",
                    RawOwner::Trap(_) => "trap",
                    RawOwner::Dialogue => "dialogue",
                    RawOwner::Shortcut(_) => "shortcut",
                    RawOwner::Death => "on_death",
                }
            ),
        }
    }
}

/// Visit **every effect root the compiler can lower**, in one fixed deterministic
/// order, as `f(&site, list)`.
///
/// The order is contractual, because emission and the l10n key scheme are defined
/// by it: per quest, `on_objective_complete` (a `BTreeMap`, so key-ordered) then
/// `on_complete`; then every trigger; then every trap payload; then every dialogue
/// `on_respawn` bundle; then every shortcut's `on_unlock`; then the campaign's
/// `on_death`. **Each new root is appended, never inserted**, so a campaign that
/// predates it produces byte-identical output — that is why R6 hangs off the
/// quests stage but comes after the dialogue-stage R5.
///
/// A list is a root if `emit::emit_quest_effect` can reach it, **not** if the
/// quests stage happens to own it. That distinction is the entire defect class:
/// six of the seven roots are reachable from `campaign.quests.content` and R5 is
/// not, so every walk reasoned from "effects live in the quests stage" was
/// correct-looking, green, and wrong. R6 is the mirror-image reading error —
/// `shortcuts[].on_unlock` *is* in `campaign.quests.content` and was still missed,
/// because the walks were written against a remembered list rather than against
/// what emission reaches.
///
/// Returns the [`RootBinding`] ledger — how many roots were enumerated and how
/// many bundles each actually bound to on this campaign. A proof that states what
/// it examined reports it; a caller that does not need it may drop it.
///
/// # Panics
///
/// If the walk failed to enumerate all [`EffectRootKind::COUNT`] roots. Unreachable
/// by construction — the macro emits one block per root — and asserted anyway, in
/// release builds too, because the failure it guards has no other symptom: a walk
/// that quietly stops visiting a root just answers a narrower question and stays
/// green over every campaign that does not use it. That is how this defect class
/// survived six independent fixes.
pub fn for_each_effect_root<'a>(
    c: &'a Campaign,
    f: &mut dyn FnMut(&EffectRootSite<'a>, &'a [QuestEffect]),
) -> RootBinding {
    let mut sites = [
        (EffectRootKind::ObjectiveComplete, 0usize),
        (EffectRootKind::QuestComplete, 0usize),
        (EffectRootKind::Trigger, 0usize),
        (EffectRootKind::TrapPayload, 0usize),
        (EffectRootKind::DialogueRespawn, 0usize),
        (EffectRootKind::ShortcutUnlock, 0usize),
        (EffectRootKind::OnDeath, 0usize),
    ];
    debug_assert_eq!(
        sites.map(|(k, _)| k),
        EffectRootKind::ALL,
        "the binding ledger's slots are EffectRootKind::ALL, in order"
    );
    let mut effects = 0usize;
    // Which roots the walk reached at all — set by `note`, independently of whether
    // this campaign has a bundle there.
    let mut enumerated = [false; EffectRootKind::COUNT];
    fn slot_of(kind: EffectRootKind) -> usize {
        EffectRootKind::ALL
            .iter()
            .position(|k| *k == kind)
            .expect("every root kind is a member of EffectRootKind::ALL")
    }

    effect_root_walk!(
        campaign: c,
        iter: iter,
        slice: as_slice,
        respawn: set_checkpoint_on_respawn,
        note: |kind: EffectRootKind| {
            enumerated[slot_of(kind)] = true;
        },
        visit: |(kind, owner, objective): (EffectRootKind, RawOwner<'a>, Option<&'a str>),
                path: String,
                key: String,
                list: &'a [QuestEffect]| {
            let owner = owner.attach(kind, objective);
            debug_assert_eq!(owner.kind(), kind, "a site's owner and kind must agree");
            let slot = slot_of(kind);
            sites[slot].1 += 1;
            effects += list.len();
            f(
                &EffectRootSite {
                    owner,
                    stage: kind.stage(),
                    path,
                    key,
                },
                list,
            );
        },
        quest_owner: |q| RawOwner::Quest(q),
        trigger_owner: |t| RawOwner::Trigger(t),
        trap_owner: |p| RawOwner::Trap(p),
        dialogue_owner: RawOwner::Dialogue,
        shortcut_owner: |s| RawOwner::Shortcut(s),
        death_owner: RawOwner::Death,
    );

    let missed: Vec<&str> = EffectRootKind::ALL
        .iter()
        .zip(enumerated)
        .filter(|(_, s)| !*s)
        .map(|(k, _)| k.label())
        .collect();
    assert!(
        missed.is_empty(),
        "for_each_effect_root enumerated {} of {} effect roots — missing: {}. A root that \
         stops being enumerated has no other symptom.",
        EffectRootKind::COUNT - missed.len(),
        EffectRootKind::COUNT,
        missed.join(", ")
    );

    RootBinding {
        roots_enumerated: EffectRootKind::COUNT,
        sites,
        effects,
    }
}

/// The callback [`for_each_effect_root_mut`] hands each root to:
/// `(kind, json_pointer_to_the_list, l10n_keybase, list)`. `'a` ties the effects
/// to the campaign borrow, so a consumer may collect them; `'f` is the callback's
/// own borrow.
pub type RootVisitorMut<'a, 'f> = dyn FnMut(EffectRootKind, &str, &str, &'a mut [QuestEffect]) + 'f;

/// The **mutable mirror** of [`for_each_effect_root`]: the identical roots, in the
/// identical order, with the same `(stage, path, key)` descriptors, exposed mutably
/// so the localization pass can rewrite player-visible strings in place.
///
/// Generated from the same [`effect_root_walk`] body, so "which lists are roots" is
/// not written twice and the pair cannot drift the way two hand-written mirrors
/// can. `&mut Campaign` cannot yield the owning `&Quest` alongside
/// `&mut [QuestEffect]` from the same quest, so this walk carries the
/// [`EffectRootKind`] rather than an [`EffectRootOwner`]. That is the only
/// difference between the two, and it is a difference in what is attached to a
/// visit — never in which roots are visited.
pub fn for_each_effect_root_mut<'a>(c: &'a mut Campaign, f: &mut RootVisitorMut<'a, '_>) {
    effect_root_walk!(
        campaign: c,
        iter: iter_mut,
        slice: as_mut_slice,
        respawn: set_checkpoint_on_respawn_mut,
        note: |_kind: EffectRootKind| {},
        visit: |(kind, _owner, _objective): (EffectRootKind, (), Option<&str>),
                path: String,
                key: String,
                list: &'a mut [QuestEffect]| {
            f(kind, &path, &key, list);
        },
        quest_owner: |_q| (),
        trigger_owner: |_t| (),
        trap_owner: |_p| (),
        dialogue_owner: (),
        shortcut_owner: |_s| (),
        death_owner: (),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The binding ledger's slots are `EffectRootKind::ALL`, in order — the
    /// property `summary()` and `unbound_roots()` both read off positionally.
    #[test]
    fn binding_slots_are_all_the_roots_in_order() {
        let b = RootBinding {
            roots_enumerated: EffectRootKind::COUNT,
            sites: [
                (EffectRootKind::ObjectiveComplete, 0),
                (EffectRootKind::QuestComplete, 0),
                (EffectRootKind::Trigger, 0),
                (EffectRootKind::TrapPayload, 0),
                (EffectRootKind::DialogueRespawn, 0),
                (EffectRootKind::ShortcutUnlock, 0),
                (EffectRootKind::OnDeath, 0),
            ],
            effects: 0,
        };
        assert_eq!(b.sites.map(|(k, _)| k), EffectRootKind::ALL);
        assert_eq!(b.unbound_roots().len(), EffectRootKind::COUNT);
    }

    /// Every root kind names a stage and a label, and the stages are exactly the
    /// two stage documents effect roots live in.
    #[test]
    fn every_root_names_its_stage() {
        for k in EffectRootKind::ALL {
            assert!(matches!(k.stage(), "quests" | "dialogue"), "{k:?}");
            assert!(!k.label().is_empty(), "{k:?}");
        }
    }
}
