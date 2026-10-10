//! Branch-coherent campaign flow model — the sound half of the ADR-0005 static
//! layer, shared by [`crate::compiler::analyze`] (reachability diagnostics) and
//! [`crate::compiler::plan`] (critical-path export).
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
//! | `set-flag` in a `traps[].payload` | the trap's `requires_flags` are satisfied (ambient, same reasoning: the party can always walk over and spring it) |
//! | a trap's `disarm.sets_flag` | the trap's `requires_flags` are satisfied (ambient, same reasoning) |
//! | a timed gate's `disarm.sets_flag` | always (ambient — the jam lever is an optional player action) |
//! | `set-flag` in an `on_respawn` / `on_caught` reaction bundle | **never** — reaction bundles fire at statically unknowable times, so they are not producers (the conservative stance [`crate::compiler::continuity`] already takes), whether the bundle is rooted in the quests stage or hung off a dialogue option's `set-checkpoint` |
//!
//! Which effect **lists** those rows range over is not this module's to decide:
//! both the producer scan in [`Flow::new`] and the reader inventory
//! ([`gate_flags`]) walk [`crate::compiler::plan::for_each_effect_root`], the single
//! enumeration of the five roots emission can lower an effect from. A
//! hand-listed subset is what lets a `set-flag` in a `traps[].payload` be no
//! producer *anywhere* in the proof while the emitted
//! `trap_fire_<trap>.mcfunction` sets it, and keeps a `requires_flags` inside
//! such a payload out of the branch model. The table above is a **policy** per
//! root; the roots themselves are inherited.
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
//!
//! ## Optional participation
//!
//! The owner's contract is that **the mainline must be completable with zero
//! optional participation**: side content may never gate it. The **mainline** is
//! the critical path [`Flow::playthrough`] exports — exactly the participation the
//! campaign requires to reach `campaign-complete`; anything else the player may do
//! is optional. [`Flow::replay`] credits only the mainline's own producers (the
//! taken dialogue option's flags, on-path bundles, and ambient trigger/trap flags a
//! player can always fire), so a mainline objective gated on a flag only an
//! off-path quest or an unselected option sets is `DW0204`. The
//! participation-minimal walk is the replay.
//!
//! There is no order half to prove any more (spec-0093 §6.3). Every objective
//! driver the compiler emits — the dialogue button that fires
//! `complete-objective` included — goes through the emitter's `pending_guard`
//! (quest active ∧ `after` complete ∧ `requires_flags` ∧ `forbids_flags` ∧
//! `requires_state` ∧ not yet complete), so a button that would walk the party
//! past a load-bearing beat is not drawn until that beat is done. The island's
//! owner-hit softlock — "Lead on." (completing `obj/climb-out`) beside "We
//! climb." (completing `obj/muster`) from campaign start — is dissolved by that
//! guard rather than reported by a rule.

use delvewright_dsl::Verb;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{
    Campaign, CompareOp, DialogueEffect, Objective, QuestEffect, StateCompare, StateScope,
    StateWrite, Trigger,
};
use delvewright_dsl::{DwCode, ExitTier};

delvewright_dsl::dw_code! {
    /// Objective-path incoherence (the replay check).
    pub const DW_PATH_INCOHERENT: DwCode = DwCode::new("DW0204", ExitTier::Analysis);
}

delvewright_dsl::dw_code! {
    /// A forced-path numeric gate the path itself has already made unsatisfiable.
    pub const DW_STATE_GATE_CLEARED: DwCode = DwCode::new("DW0879", ExitTier::Analysis);
}

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
    /// The environment trigger whose effects produce it, when it is one. A
    /// trigger is ambient only in the sense that nothing on the quest DAG orders
    /// it: somebody still has to strike, use, approach or hit it, and
    /// [`Flow::trigger_debts`] is where the path is made to say who and when.
    trigger: Option<String>,
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

/// A dialogue node some quest's `cast` ledger opens for an NPC — the second
/// kind of entry point a body offers, beside the tree's own stage-6 `root`.
///
/// A ledger root is not a shortcut into the tree: right-click opens it
/// *directly* for that quest's duration, so a node whose only entry is one of
/// these is reachable, and the flags its options set are producible. That is
/// the whole point of the ledger (spec-0020) — an NPC's right-click being a
/// different scene per quest.
#[derive(Clone, Debug)]
struct CastRoot {
    /// The quest whose ledger declares it: the root is live only once this
    /// quest has begun.
    quest: String,
    /// The node id the right-click opens.
    root: String,
    /// The placement's own `requires_flags`. A per-branch cast clause is gated
    /// on the branch's flag, which is what makes a ledger root open a node in
    /// **some worlds and not others**.
    requires: Vec<String>,
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
/// objectives/quests are done, which quests are active, and what each declared
/// datum holds.
#[derive(Clone, Debug, Default)]
struct ReplayState {
    flags: BTreeSet<String>,
    done_obj: BTreeSet<String>,
    done_quest: BTreeSet<String>,
    active: BTreeSet<String>,
    /// Every declared datum's value at this point of the walk. A datum no
    /// ordered walk can date is [`Datum::Undatable`] from the start and stays
    /// that way — see [`Flow::undatable`].
    state: BTreeMap<String, Datum>,
    /// Per datum, the writes this walk has applied to it, in the order it
    /// applied them. The blame a refusal names.
    wrote: BTreeMap<String, Vec<StateWriteRecord>>,
    /// Writes to a datum this walk deliberately did not apply, in path order —
    /// a write whose own gate was open to question at the moment it was reached.
    /// Their presence is what withholds a refusal, so they are recorded rather
    /// than dropped.
    unapplied: BTreeSet<String>,
    /// The JSON pointer of every effect the last step fired — its gate held
    /// where the replay reached it — in firing order. Cleared at each step.
    fired: Vec<String>,
    /// The subset of `fired` whose gate held only because a numeric term read
    /// an undatable datum: the replay walks on through it (withholding a
    /// producer is the unsound direction there), and a proof that asks whether
    /// the line is FORCED must not read it as such. Cleared at each step.
    undecided: Vec<String>,
}

/// One datum's value during a replay.
///
/// **`Undatable` is not "we did not look".** It is the model saying that no
/// ordered walk of the campaign can name this datum's value at a given beat,
/// because something writes it at a moment nothing in the document orders — an
/// ambient producer the party may fire any number of times, a reaction bundle
/// that runs when somebody dies, or a stake a death forfeits. The conservative
/// direction is the only sound one here: a comparison against an undatable
/// value is never refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Datum {
    /// The walk knows this value exactly.
    Known(i64),
    /// No ordered walk can name it — see [`Flow::undatable`].
    Undatable,
}

impl Datum {
    /// Does this value satisfy `cmp`? `None` when the value is undatable, which
    /// is the only answer that is neither yes nor no.
    fn satisfies(self, cmp: &StateCompare) -> Option<bool> {
        let Datum::Known(v) = self else {
            return None;
        };
        Some(i32::try_from(v).is_ok_and(|v| cmp.holds(v)))
    }

    /// The value after `w` is applied. `initial` is the datum's declared
    /// starting value, which is also what `clear-state` returns it to.
    ///
    /// **Undatable absorbs, including under `set-state` and `clear-state`.** A
    /// write that pins a value pins it only until the next undated write, and an
    /// undated write is by definition one that can land at any moment — the one
    /// after this beat included. A `set-state 2` on a datum a death forfeits
    /// says nothing about what the datum holds four beats later, so treating it
    /// as freshly known would manufacture exactly the certainty
    /// [`Flow::undatable`] exists to refuse.
    fn write(self, w: StateWrite, initial: i64) -> Datum {
        let Datum::Known(v) = self else {
            return Datum::Undatable;
        };
        match w {
            StateWrite::Set(n) => Datum::Known(i64::from(n)),
            StateWrite::Clear => Datum::Known(initial),
            StateWrite::Add(n) => Datum::Known(v + i64::from(n)),
        }
    }
}

/// One write a replay applied to a datum: which beat performed it, what it did,
/// and what the datum held afterwards.
#[derive(Clone, Debug, PartialEq, Eq)]
struct StateWriteRecord {
    /// The beat whose bundle carried the write.
    beat: Beat,
    /// 1-based path position of that beat.
    position: usize,
    /// The verb, for the message (`set-state` / `add-state` / `clear-state`).
    verb: &'static str,
    /// What the datum held once the write had been applied.
    after: Datum,
}

/// A beat that can write a datum during a replay — the two dated effect roots.
///
/// This is the vocabulary the ORDER is expressed in: a refusal is only sound
/// where every write the walk applied is forced, by the campaign's own `after`
/// and `quest-complete` chains, to happen before the gate that reads it. See
/// [`Flow::forced_before`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Beat {
    /// A quest's `on_objective_complete[<objective>]` bundle.
    Objective(String),
    /// A quest's `on_complete` bundle — after every objective of that quest, and
    /// before every beat of every quest it triggers.
    QuestComplete(String),
    /// An environment trigger's `effects`, fired where the path performs it
    /// ([`Walk::probe`]).
    Trigger(String),
}

impl Beat {
    /// How a message names this beat.
    fn phrase(&self) -> String {
        match self {
            Beat::Objective(o) => format!("`{o}`'s completion bundle"),
            Beat::QuestComplete(q) => format!("`{q}`'s `on_complete` bundle"),
            Beat::Trigger(t) => format!("trigger `{t}`'s effects"),
        }
    }
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

/// One flag the exported path reads that only an environment trigger produces,
/// and the trigger the path performs to produce it ([`Flow::trigger_debts`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriggerDebt {
    /// Index into [`Playthrough::steps`] of the first step that reads the flag.
    pub step: usize,
    /// The trigger the path performs, before that step, to pay it.
    pub trigger: String,
    /// The flag it pays.
    pub flag: String,
}

/// One step of a branch's compiled play order (spec-0025), with the state
/// transition it caused.
#[derive(Clone, Debug)]
pub struct JournalStep {
    /// The quest the objective belongs to.
    pub quest: String,
    /// The objective completed at this step.
    pub objective: String,
    /// For a `talk-to`, the flat dialogue option index taken.
    pub talk_option: Option<usize>,
    /// Quests that became active at this step.
    pub opened: Vec<String>,
    /// Quests that completed at this step.
    pub completed: Vec<String>,
    /// Flags held when the step began.
    pub flags_before: BTreeSet<String>,
    /// Flags held once the step's bundles have fired.
    pub flags_after: BTreeSet<String>,
    /// The JSON pointer of every effect this step fired — the objective's
    /// bundle and the `on_complete` of every quest it completed, nested lists
    /// included — read by the replay's own gate test where it reached each one.
    /// The one answer to "did this beat play", which the chronicle reads.
    pub fired: BTreeSet<String>,
    /// The members of `fired` whose gate held only on an undatable numeric
    /// term — played by the replay, never forced (`plan::firing_of`).
    pub undecided: BTreeSet<String>,
}

/// One play order under construction ([`Flow::walk`]): the replay state
/// machine, advanced one chosen step at a time. Cloning it is how a caller
/// looks one step ahead.
#[derive(Clone)]
pub struct Walk<'f, 'a> {
    flow: &'f Flow<'a>,
    st: ReplayState,
    complete_at: Option<(usize, String)>,
    /// Steps taken so far.
    taken: usize,
    /// Which ambient producers this walk credits. `None` is the replay's own
    /// stance — every producer whose gate holds, because a player CAN fire it.
    /// `Some(performed)` is the guaranteed stance ([`Flow::walk_performing`]):
    /// only the producers of the triggers the path has performed so far, and no
    /// other ambient producer at all (a trap disarm, a timed gate's disarm, a
    /// purchase are acts nothing forces).
    performed: Option<BTreeSet<String>>,
}

impl Walk<'_, '_> {
    /// Whether this walk credits ambient producer `g` now.
    fn credits(&self, g: &GatedFlag) -> bool {
        match &self.performed {
            None => true,
            Some(done) => g.trigger.as_ref().is_some_and(|t| done.contains(t)),
        }
    }

    /// The path performs trigger `id` here: from now on its producers are
    /// credited (to fixpoint, under their own gates). A no-op on a walk that
    /// already credits every producer.
    pub fn perform(&mut self, id: &str) {
        let Some(done) = self.performed.as_mut() else {
            return;
        };
        done.insert(id.to_string());
        let mut flags = std::mem::take(&mut self.st.flags);
        self.flow
            .saturate_ambient_with(&mut flags, &|g: &GatedFlag| self.credits(g));
        self.st.flags = flags;
    }

    /// Can `step` be completed now — the replay's own per-step test, and not
    /// already done.
    pub fn legal(&self, step: &PathStep) -> bool {
        !self.st.done_obj.contains(&step.objective)
            && self.flow.step_blocked(&self.st, step).is_none()
    }

    /// Complete `step` (its guards are the caller's to have checked) and
    /// return the journal entry for the transition.
    pub fn take(&mut self, step: &PathStep) -> JournalStep {
        let before = self.st.clone();
        self.taken += 1;
        let mut st = std::mem::take(&mut self.st);
        let mut complete_at = self.complete_at.take();
        self.flow.advance_with(
            &mut st,
            step,
            self.taken,
            &mut complete_at,
            &|g: &GatedFlag| self.credits(g),
        );
        self.st = st;
        self.complete_at = complete_at;
        JournalStep {
            quest: step.quest.clone(),
            objective: step.objective.clone(),
            talk_option: step.talk_option,
            opened: self.st.active.difference(&before.active).cloned().collect(),
            completed: self
                .st
                .done_quest
                .difference(&before.done_quest)
                .cloned()
                .collect(),
            flags_before: before.flags,
            flags_after: self.st.flags.clone(),
            fired: self.st.fired.iter().cloned().collect(),
            undecided: self.st.undecided.iter().cloned().collect(),
        }
    }

    /// Fire `effs` — an environment trigger's `effects`, whose root pointer is
    /// `base` — against the state this walk holds NOW, without advancing it, and
    /// return what fired and which of those held only on an undatable term.
    ///
    /// The same [`Flow::fire`] every quest bundle goes through, so a trigger's
    /// line is judged by the one gate test the replay applies everywhere.
    pub fn probe(
        &self,
        trigger: &str,
        effs: &[QuestEffect],
        base: &str,
    ) -> (BTreeSet<String>, BTreeSet<String>) {
        let mut st = self.st.clone();
        st.fired.clear();
        st.undecided.clear();
        let mut complete_at = self.complete_at.clone();
        self.flow.fire(
            effs,
            base,
            &mut st,
            &mut complete_at,
            self.taken,
            "",
            &Beat::Trigger(trigger.to_string()),
        );
        (
            st.fired.into_iter().collect(),
            st.undecided.into_iter().collect(),
        )
    }

    /// Has `campaign-complete` fired? Nothing is walked after it.
    pub fn ended(&self) -> bool {
        self.complete_at.is_some()
    }

    /// Has `objective` been completed on this walk?
    pub fn done(&self, objective: &str) -> bool {
        self.st.done_obj.contains(objective)
    }

    /// The flags held now.
    pub fn flags(&self) -> &BTreeSet<String> {
        &self.st.flags
    }

    /// Every declared datum's value now, by id — `None` where no ordered walk
    /// can name it ([`Flow::undatable`]).
    pub fn data(&self) -> Vec<(&str, Option<i64>)> {
        self.st
            .state
            .iter()
            .map(|(id, d)| {
                let v = match d {
                    Datum::Known(v) => Some(*v),
                    Datum::Undatable => None,
                };
                (id.as_str(), v)
            })
            .collect()
    }

    /// The quests running now.
    pub fn active_quests(&self) -> &BTreeSet<String> {
        &self.st.active
    }

    /// What a right-click on each NPC can put in front of the player now: every
    /// node the live scene opens or the drawn options lead to, and every option
    /// drawn on those nodes. `casts` is [`crate::compiler::cast::npc_casts`] of the
    /// same campaign, resolved once by the caller.
    pub fn dialogue_on_screen(
        &self,
        casts: &BTreeMap<String, crate::compiler::cast::NpcCast>,
    ) -> DialogueOnScreen {
        self.flow.dialogue_on_screen(casts, &self.st, None)
    }

    /// [`Self::dialogue_on_screen`] just after `step` was taken by its dialogue
    /// option: the option's `next` node is shown at the state its own effects
    /// produced, so what that node draws counts too — a button the click itself
    /// made pending is on screen at once, even where no right-click reaches the
    /// node any more.
    pub fn dialogue_on_screen_after(
        &self,
        casts: &BTreeMap<String, crate::compiler::cast::NpcCast>,
        step: &PathStep,
    ) -> DialogueOnScreen {
        let opened = self.flow.talk_next(step);
        self.flow.dialogue_on_screen(
            casts,
            &self.st,
            opened.as_ref().map(|(n, d)| (n.as_str(), d.as_str())),
        )
    }
}

/// The dialogue a player can read at one walk state ([`Walk::dialogue_on_screen`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DialogueOnScreen {
    /// `(npc, node id)` of every node whose text a right-click can bring up.
    pub nodes: BTreeSet<(String, String)>,
    /// `(npc, node id, option index within the node)` of every option drawn.
    pub options: BTreeSet<(String, String, usize)>,
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
    /// The `DW0204` diagnostic message, about the exported critical path.
    pub fn message(&self) -> String {
        self.message_on("the exported critical path")
    }

    /// The `DW0204` diagnostic message about `path`: the critical path, or one
    /// branch's exported path. Every path the build exports is walked, so every
    /// one of them answers to the same rule.
    pub fn message_on(&self, path: &str) -> String {
        format!(
            "{path} is not a playthrough any player can walk: at path step \
             #{} (objective `{}`), {}. Every step of the critical path must be activatable and \
             completable at its position, and `campaign-complete` must fire exactly at the final \
             step — split mutually exclusive endings behind flags so exactly one of them lies on \
             the path, and make each gating flag producible before the step that reads it",
            self.position, self.objective, self.reason
        )
    }
}

/// A forced-path numeric gate the path itself has already made unsatisfiable
/// (`DW0879`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateGateCleared {
    /// 1-based position of the objective on the path that was walked.
    pub position: usize,
    /// The quest the objective belongs to.
    pub quest: String,
    /// The objective whose gate cannot hold.
    pub objective: String,
    /// The datum the gate reads.
    pub state: String,
    /// The comparison, as written.
    pub op: CompareOp,
    /// What the comparison compares against.
    pub value: i32,
    /// What the replay holds at the moment the gate is read.
    pub held: i64,
    /// The write that left it holding that, and where on the path it happened.
    /// `None` when nothing on the path wrote the datum at all and the value is
    /// still the declared `initial`.
    pub by: Option<StateBlame>,
    /// The branch this was proven on, when it is branch-specific; `None` for the
    /// campaign's own critical path.
    pub branch: Option<String>,
}

/// The write a [`StateGateCleared`] blames, phrased for a reader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateBlame {
    /// The beat whose bundle carried the write.
    beat: Beat,
    /// 1-based path position of that beat.
    pub position: usize,
    /// The verb (`set-state` / `add-state` / `clear-state`).
    pub verb: &'static str,
}

impl StateBlame {
    /// The beat, as the message names it.
    pub fn beat(&self) -> String {
        self.beat.phrase()
    }
}

impl StateGateCleared {
    /// The `DW0879` diagnostic message.
    pub fn message(&self) -> String {
        let on = match &self.branch {
            Some(b) => format!(" on branch `{b}`"),
            None => String::new(),
        };
        let blame = match &self.by {
            Some(b) => format!(
                "{beat} performed a `{verb}` on it at step #{at}, and nothing between there and \
                 here writes it again",
                beat = b.beat(),
                verb = b.verb,
                at = b.position,
            ),
            None => "nothing on the path writes it at all, so it still holds its declared \
                     `initial`"
                .to_string(),
        };
        format!(
            "objective `{obj}` is gated on `{state} {op} {value}`, and at its own position on the \
             path{on} (step #{pos}) `{state}` holds {held} — so the gate cannot open, the beat \
             never activates, and every beat after it is unreachable. {blame}. The campaign's own \
             `after` and `quest-complete` chains force that write to happen before this gate, so \
             no play order avoids it: this is not a beat the party can come back to. Move the \
             write past the beat that reads the datum, or move the gate — a `clear-state` standing \
             between a datum's producer and its reader empties it for every gate downstream, and \
             the static checks around this one cannot see it (`DW0501` asks whether a datum is \
             written ANYWHERE, and `DW0527` asks about one bundle's own effect list)",
            obj = self.objective,
            state = self.state,
            op = self.op.token(),
            value = self.value,
            pos = self.position,
            held = self.held,
        )
    }
}

/// What one [`Flow::state_gates`] walk examined — the binding count a run states
/// whether or not anything was found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateWalk {
    /// Declared data the walk carried a value for.
    pub data: usize,
    /// Of those, the ones no ordered walk can date (see [`Datum::Undatable`]).
    pub undatable: usize,
    /// Path steps walked.
    pub steps: usize,
    /// Comparison terms on those steps' objective gates that this walk read.
    pub gates: usize,
    /// State writes the walk applied.
    pub writes: usize,
    /// Gate terms that failed and were nonetheless NOT refused, because the
    /// writes before them are not forced into one order — see
    /// [`Flow::forced_before`].
    pub withheld: usize,
    /// Gate terms read against an **undatable** datum, which this walk can
    /// neither hold nor refuse.
    ///
    /// Counted apart from [`Self::gates`] on purpose: a run that read twenty
    /// terms and decided all twenty, and one that read twenty and could decide
    /// none, print the same `gates` figure, and the second is the one where a
    /// green means nothing. The pair is what separates them.
    pub undated: usize,
}

impl StateWalk {
    /// Fold another walk's counts in — the per-path counts add, and `data` /
    /// `undatable` are campaign-wide facts, so the largest stands.
    pub fn merge(&mut self, other: StateWalk) {
        self.data = self.data.max(other.data);
        self.undatable = self.undatable.max(other.undatable);
        self.steps += other.steps;
        self.gates += other.gates;
        self.writes += other.writes;
        self.withheld += other.withheld;
        self.undated += other.undated;
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
    /// Every `cast` ledger dialogue root, by NPC — the entry points the tree's
    /// own `root` does not name.
    cast_roots: BTreeMap<String, Vec<CastRoot>>,
    /// Producers with no DAG position: environment triggers and trap disarms.
    ambient: Vec<GatedFlag>,
    obj_flags: BTreeMap<String, Vec<GatedFlag>>,
    quest_flags: BTreeMap<String, Vec<GatedFlag>>,
    worlds: Vec<World>,
    /// Every declared datum's starting value — what it holds before anything
    /// happens, and what `clear-state` returns it to.
    initial: BTreeMap<String, i64>,
    /// The data no ordered walk can date, and therefore the data no comparison
    /// is ever refused against. See [`Datum::Undatable`] for the reasoning; the
    /// three sources are named where the set is built.
    undatable: BTreeSet<String>,
}

impl<'a> Flow<'a> {
    /// Build the model. Pure and deterministic (BTree ordering throughout).
    pub fn new(c: &'a Campaign) -> Self {
        let trees = flatten_trees(c);
        let (groups, opt_group) = choice_groups(&trees);
        let mut ambient = Vec::new();
        let mut obj_flags: BTreeMap<String, Vec<GatedFlag>> = BTreeMap::new();
        let mut quest_flags: BTreeMap<String, Vec<GatedFlag>> = BTreeMap::new();
        // The producer model, root by root. The roots come from
        // `plan::for_each_effect_root` — the ONE enumeration of what emission can
        // lower — so the proof cannot believe in fewer firings than the datapack
        // performs. What each root means is the policy stated here,
        // once, and the match is exhaustive: a sixth root cannot be added without
        // this file deciding what it is.
        let gate_of = |fs: &[delvewright_dsl::FlagId]| -> Vec<String> {
            fs.iter().map(|f| f.as_str().into()).collect()
        };
        crate::compiler::plan::for_each_effect_root(c, &mut |site, effs| match site.root {
            // Dated by the DAG: credited when that objective / quest is proven
            // reachable, under the gates on its own effect chain.
            crate::compiler::plan::EffectRoot::ObjectiveComplete { objective, .. } => {
                let mut out = Vec::new();
                collect_flags(effs, &[], &mut out);
                if !out.is_empty() {
                    obj_flags
                        .entry(objective.to_string())
                        .or_default()
                        .extend(out);
                }
            }
            crate::compiler::plan::EffectRoot::QuestComplete(q) => {
                let mut out = Vec::new();
                collect_flags(effs, &[], &mut out);
                if !out.is_empty() {
                    quest_flags.insert(q.id.as_str().to_string(), out);
                }
            }
            // Ambient: player-initiated, no DAG position. A trap payload joins the
            // environment trigger and the trap `disarm` it already sits beside —
            // the party can always walk over and spring it, which is the same
            // reason `strike`/`use`/`approach` are producers.
            crate::compiler::plan::EffectRoot::Trigger(t) => {
                collect_flags_from(
                    effs,
                    &gate_of(&t.requires_flags),
                    Some(t.id.as_str()),
                    &mut ambient,
                );
            }
            crate::compiler::plan::EffectRoot::TrapPayload(trap) => {
                collect_flags(effs, &gate_of(&trap.requires_flags), &mut ambient);
            }
            // A shortcut's `on_unlock` is ambient for the same reason, and the
            // reachability of its firing is not an assumption here — `DW0373`
            // proves the far-side `unlock` is walkable while the gate is still
            // sealed, so "the party can always go and pull it" is a theorem this
            // build has already discharged. It declares no flag gate, so the
            // producer is ungated.
            crate::compiler::plan::EffectRoot::ShortcutUnlock => {
                collect_flags(effs, &[], &mut ambient);
            }
            // A shop offer is ambient too, and its producer IS gated — by the
            // offer's own gate, which is the shared gate every other consumer
            // carries. The flag half of it is what `collect_flags` wants; the
            // numeric half is a runtime balance no static flag model can date, so
            // an offer with a price is credited as an ambient producer behind its
            // flags only, which is the conservative direction (a flag it may never
            // afford is still not assumed by the mainline — the buyer must reach
            // the shop, and reaching it is an ordinary route the completability
            // proof already owns).
            crate::compiler::plan::EffectRoot::ShopOffer => {
                collect_flags(effs, &[], &mut ambient);
            }
            // Reaction bundles: they fire only when somebody dies, at a time no
            // static model can name, so nothing inside either is a producer.
            // Exactly what `collect_flags` already refuses for the identical
            // bundle rooted in the quests stage — reached here, not credited here.
            // Crediting `on_death` would be worse than a missing producer: it
            // would let the mainline be proven reachable via a flag the party only
            // obtains by dying.
            //
            // A fight's `on_kill` (spec-0074) is the same shape: it fires only
            // when a player is CREDITED with a body, and a body may fall, burn or
            // be cut down by another mob with nobody credited, so no kill is
            // forced and nothing inside is a producer.
            //
            // An assembly's `on_land` (spec-0082) is the same shape again: a blow
            // lands only on a player who chose to stand where it reaches, so no
            // landing is forced and nothing inside is a producer.
            crate::compiler::plan::EffectRoot::DialogueRespawn
            | crate::compiler::plan::EffectRoot::OnDeath
            | crate::compiler::plan::EffectRoot::OnKill(_)
            | crate::compiler::plan::EffectRoot::AssemblyLand(_)
            // A loop's `on_cross` (spec-0086) runs on a crossing, which a mob makes
            // as readily as a player: no ordered walk dates it, so nothing inside
            // is a producer the mainline may rely on.
            | crate::compiler::plan::EffectRoot::LoopCross(_) => {}
        });
        // `disarm.sets_flag` is a field, not an effect list, so it has no root of
        // its own; same ambient reasoning, same gate.
        for trap in &c.quests.content.traps {
            if let Some(d) = &trap.disarm {
                ambient.push(GatedFlag {
                    flag: d.sets_flag.as_str().to_string(),
                    requires: gate_of(&trap.requires_flags),
                    trigger: None,
                });
            }
        }
        // A timed gate's disarm produces its flag the same way — an
        // optional player action nothing orders, so it is ambient, ungated.
        for g in &c.quests.content.timed_gates {
            if let Some(d) = &g.disarm {
                ambient.push(GatedFlag {
                    flag: d.sets_flag.as_str().to_string(),
                    requires: Vec::new(),
                    trigger: None,
                });
            }
        }

        let worlds = enumerate_worlds(&groups, &gate_flags(c));
        let initial: BTreeMap<String, i64> = c
            .quests
            .content
            .state
            .iter()
            .map(|s| (s.id.as_str().to_string(), i64::from(s.initial)))
            .collect();
        Flow {
            c,
            groups,
            opt_group,
            trees,
            cast_roots: cast_roots(c),
            ambient,
            obj_flags,
            quest_flags,
            worlds,
            initial,
            undatable: undatable_state(c),
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
        self.playthrough_from(sol, false)
    }

    /// How many branch worlds the model enumerated.
    pub fn world_count(&self) -> usize {
        self.worlds.len()
    }

    /// The flags producible in world `i` (spec-0025: the branch-assignment
    /// consistency test reads this).
    pub fn world_flags(&self, i: usize) -> BTreeSet<String> {
        self.solve(&self.worlds[i]).flags
    }

    /// The quests that complete in world `i`.
    pub fn world_completed(&self, i: usize) -> BTreeSet<String> {
        self.solve(&self.worlds[i]).completed
    }

    /// [`Self::playthrough`] restricted to one enumerated world — the branch's
    /// own critical path (spec-0025). Unlike `playthrough` it does not search for
    /// a finale-completing world: the caller has already chosen the world that
    /// realizes the branch's declared flag assignment.
    pub fn playthrough_in(&self, i: usize) -> Playthrough {
        self.playthrough_from(Some(self.solve(&self.worlds[i])), true)
    }

    fn playthrough_from(&self, sol: Option<Solution>, whole_world: bool) -> Playthrough {
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
        // The exported critical path is rooted at the finale (everything the
        // ending depends on). A **branch** playthrough is rooted at the branch
        // instead: a branch that runs to its own ending never completes the
        // stage-4 `finale`, so rooting there would say the branch plays nothing
        // at all (spec-0025).
        let (order, cyclic) = if whole_world {
            dag_order(self.c, &keep)
        } else {
            finale_order(self.c, &keep)
        };
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

    /// Replay `p` and record, per step, the quests that opened, the quests that
    /// completed, and the flag state on either side of it — the **compiled play
    /// order** the per-branch chronicle and the contradiction proof are assembled
    /// from (spec-0025).
    ///
    /// It is deliberately the SAME state machine [`Self::replay`] proves against
    /// (one [`Self::advance`], one [`Self::fire`]), so the chronicle can never
    /// describe an order the replay does not admit.
    pub fn journal(&self, p: &Playthrough) -> Vec<JournalStep> {
        let mut w = self.walk();
        p.steps.iter().map(|step| w.take(step)).collect()
    }

    /// **The flags `p` owes to an environment trigger, and which trigger pays
    /// each one** — the flag half of the path-performs-its-triggers rule.
    ///
    /// The replay credits a trigger's `set-flag` the moment the trigger's own flag
    /// gate holds, because a player *can* fire it then. The exported path is what a
    /// bot walks, and a bot only does what a step tells it: a flag the path reads
    /// that only a trigger produces is a party action the path must contain, or the
    /// walk the replay proved is a walk nobody makes.
    ///
    /// This is the same state machine ([`Self::advance`]) with trigger-rooted
    /// producers withheld until the path performs them. At each step, every flag
    /// the step reads — its objective's `requires_flags`, and for a `talk-to` the
    /// taken option's own `requires` — that the withheld walk does not hold is a
    /// debt; it is paid by the first declared trigger that produces the flag and
    /// whose gate holds there, and from then on that trigger's producers are
    /// credited. A debt no trigger can pay is not reported here: the unwithheld
    /// replay (`DW0204`) owns every flag nothing produces.
    ///
    /// Returned in path order; `step` indexes `p.steps`.
    pub fn trigger_debts(&self, p: &Playthrough) -> Vec<TriggerDebt> {
        let mut performed: BTreeSet<String> = BTreeSet::new();
        let mut complete_at: Option<(usize, String)> = None;
        let mut st = self.initial_state_with(&|g: &GatedFlag| g.trigger.is_none());
        let mut out: Vec<TriggerDebt> = Vec::new();
        for (i, step) in p.steps.iter().enumerate() {
            let mut reads: Vec<String> = self
                .objective(&step.objective)
                .map(|o| {
                    o.requires_flags()
                        .iter()
                        .map(|f| f.as_str().to_string())
                        .collect()
                })
                .unwrap_or_default();
            if let (Some(n), Some(Objective::TalkTo { npc, .. })) =
                (step.talk_option, self.objective(&step.objective))
                && let Some(t) = self.trees.iter().find(|t| t.npc == npc.as_str())
                && let Some(o) = t.options.iter().find(|o| o.n == n)
            {
                reads.extend(o.requires.iter().cloned());
            }
            for flag in reads {
                if st.flags.contains(&flag) {
                    continue;
                }
                let payer = self.ambient.iter().find_map(|g| {
                    let t = g.trigger.as_ref()?;
                    (g.flag == flag
                        && !performed.contains(t)
                        && g.requires.iter().all(|f| st.flags.contains(f)))
                    .then(|| t.clone())
                });
                let Some(trigger) = payer else { continue };
                performed.insert(trigger.clone());
                out.push(TriggerDebt {
                    step: i,
                    trigger,
                    flag,
                });
                let paid = &performed;
                self.saturate_ambient_with(&mut st.flags, &|g: &GatedFlag| {
                    g.trigger.as_ref().is_none_or(|t| paid.contains(t))
                });
            }
            let paid = &performed;
            self.advance_with(&mut st, step, i + 1, &mut complete_at, &|g: &GatedFlag| {
                g.trigger.as_ref().is_none_or(|t| paid.contains(t))
            });
        }
        out
    }

    /// Replay `p`'s step sequence through the flag/objective/quest state machine.
    /// `Ok(())` means every step is activatable and completable at its position
    /// and `campaign-complete` fires exactly at the final step.
    pub fn replay(&self, p: &Playthrough) -> Result<(), ReplayFailure> {
        let mut st8 = self.initial_state();
        let mut complete_at: Option<(usize, String)> = None;

        for (i, st) in p.steps.iter().enumerate() {
            let pos = i + 1;
            if let Some(reason) = self.step_blocked(&st8, st) {
                return Err(ReplayFailure {
                    position: pos,
                    objective: st.objective.clone(),
                    reason,
                });
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

    /// **The replay's per-step test**: why `step` cannot be completed at `st8`,
    /// or `None` when it can — its quest active, its `after` prerequisites done,
    /// its `requires_flags` held and its `forbids_flags` clear, and for a
    /// `talk-to` the completing option this world takes reachable now. The one
    /// definition of "legal at this position", read by [`Self::replay`] and by
    /// every [`Walk`].
    fn step_blocked(&self, st8: &ReplayState, st: &PathStep) -> Option<String> {
        let Some(quest) = self.quest(&st.quest) else {
            return Some(format!("its quest `{}` does not exist", st.quest));
        };
        let Some(obj) = quest
            .objectives
            .iter()
            .find(|o| o.id().as_str() == st.objective)
        else {
            return Some("it is not an objective of its quest".to_string());
        };
        if !st8.active.contains(&st.quest) {
            return Some(format!(
                "its quest `{}` is not active yet — nothing earlier on the path completes the \
                 quest its trigger names",
                st.quest
            ));
        }
        for a in obj.after() {
            if !st8.done_obj.contains(a.as_str()) {
                return Some(format!(
                    "its `after` prerequisite `{}` has not been completed at this point",
                    a.as_str()
                ));
            }
        }
        for f in obj.requires_flags() {
            if !st8.flags.contains(f.as_str()) {
                return Some(format!(
                    "it requires `{}`, which nothing earlier on the path sets (a mutually \
                     exclusive branch produces it)",
                    f.as_str()
                ));
            }
        }
        for f in obj.forbids_flags() {
            if st8.flags.contains(f.as_str()) {
                return Some(format!(
                    "it forbids `{}`, which an earlier step on the path has already set",
                    f.as_str()
                ));
            }
        }
        if let Objective::TalkTo { npc, .. } = obj {
            let Some(n) = st.talk_option else {
                return Some(format!(
                    "no dialogue option of `{}` completes it in the chosen branch",
                    npc.as_str()
                ));
            };
            if !self.option_takeable_now(npc.as_str(), n, st8) {
                return Some(format!(
                    "the completing dialogue option of `{}` is not reachable at this point — \
                     a node or option on the way to it is still flag-gated",
                    npc.as_str()
                ));
            }
        }
        None
    }

    /// Every ambient producer (environment trigger, trap payload, trap disarm):
    /// the flag it sets and the flags the chain that sets it requires. The
    /// replay credits each one the moment its requirements hold.
    pub fn ambient_producers(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.ambient
            .iter()
            .map(|g| (g.flag.as_str(), g.requires.as_slice()))
    }

    /// A walk from the start of the delve, one step at a time, through the
    /// replay state machine — for a caller that chooses the order itself rather
    /// than replaying a [`Playthrough`] (spec-0025: `DW0485` asks its question
    /// over every order a branch admits, not only the exported one).
    pub fn walk(&self) -> Walk<'_, 'a> {
        Walk {
            flow: self,
            st: self.initial_state(),
            complete_at: None,
            taken: 0,
            performed: None,
        }
    }

    /// [`Self::walk`] under the **guaranteed** stance: an ambient producer is
    /// credited only once the path performs its trigger ([`Walk::perform`]), and
    /// a producer no trigger owns is never credited. The same state machine
    /// ([`Self::advance_with`], [`Self::saturate_ambient_with`]) the replay and
    /// [`Self::trigger_debts`] run, with the one predicate that says which
    /// producers have fired — so "this flag is set at this step" has one model,
    /// asked at the strength the question needs. Read by the forcing rule
    /// (`plan::PathFiring`).
    pub fn walk_performing(&self) -> Walk<'_, 'a> {
        Walk {
            flow: self,
            st: self.initial_state_with(&|_| false),
            complete_at: None,
            taken: 0,
            performed: Some(BTreeSet::new()),
        }
    }

    /// **Every numeric gate on `p`, read at the position it is read at**
    /// (`DW0879`).
    ///
    /// The replay is the only thing in this model with a notion of *when*: the
    /// fixpoint in [`Self::solve`] is monotone and has no position to evaluate a
    /// comparison at, which is exactly why this rule cannot live there. Walking
    /// `p` with [`Self::advance`] — the one transition function — carries each
    /// declared datum's value forward through every write the path performs, and
    /// at each objective asks whether the gate that objective declares can hold
    /// *there*.
    ///
    /// **A refusal is withheld unless the order is forced.** A player may
    /// complete two objectives with no `after` between them in either order, so
    /// a gate that fails under the exported order and holds under another is not
    /// a defect — it is a path this walk happened to pick. So a term is refused
    /// only when every write the walk applied to that datum is chained into one
    /// order by the campaign's own `after` and `quest-complete` relations, and
    /// that chain ends before the gate ([`Self::forced_before`]). Where a write
    /// to the datum was reached and NOT applied, the value depends on more than
    /// the order and the term is withheld too. Both are counted in
    /// [`StateWalk::withheld`] rather than dropped.
    ///
    /// `branch` names the playthrough for a reader; `None` is the campaign's own
    /// critical path.
    pub fn state_gates(
        &self,
        p: &Playthrough,
        branch: Option<&str>,
    ) -> (Vec<StateGateCleared>, StateWalk) {
        let mut walk = StateWalk {
            data: self.initial.len(),
            undatable: self.undatable.len(),
            steps: p.steps.len(),
            ..StateWalk::default()
        };
        let mut out: Vec<StateGateCleared> = Vec::new();
        if self.initial.is_empty() {
            return (out, walk);
        }
        let ancestors = self.quest_ancestors();
        let mut st = self.initial_state();
        let mut complete_at: Option<(usize, String)> = None;
        for (i, step) in p.steps.iter().enumerate() {
            let pos = i + 1;
            if let Some(obj) = self.objective(&step.objective) {
                for cmp in obj.requires_state() {
                    walk.gates += 1;
                    let id = cmp.state.as_str();
                    let held = st.state.get(id).copied().unwrap_or(Datum::Undatable);
                    match held.satisfies(cmp) {
                        Some(true) => continue,
                        None => {
                            walk.undated += 1;
                            continue;
                        }
                        Some(false) => {}
                    }
                    let applied: &[StateWriteRecord] =
                        st.wrote.get(id).map(|v| v.as_slice()).unwrap_or(&[]);
                    if st.unapplied.contains(id)
                        || !self.forced_before(applied, &step.objective, &ancestors)
                    {
                        walk.withheld += 1;
                        continue;
                    }
                    // `satisfies` answered `Some(false)`, which only a `Known`
                    // value can, so the value is there to name in the message.
                    let Datum::Known(v) = held else {
                        unreachable!("an undatable value answers None, not Some(false)")
                    };
                    out.push(StateGateCleared {
                        position: pos,
                        quest: step.quest.clone(),
                        objective: step.objective.clone(),
                        state: id.to_string(),
                        op: cmp.op,
                        value: cmp.value,
                        held: v,
                        by: applied.last().map(|w| StateBlame {
                            beat: w.beat.clone(),
                            position: w.position,
                            verb: w.verb,
                        }),
                        branch: branch.map(str::to_string),
                    });
                }
            }
            self.advance(&mut st, step, pos, &mut complete_at);
        }
        walk.writes = st.wrote.values().map(Vec::len).sum();
        (out, walk)
    }

    /// Are `applied` — the writes this walk performed on one datum, in the order
    /// it performed them — forced into exactly that order, and forced to finish
    /// before `objective` activates?
    ///
    /// The relation is the campaign's own, and nothing weaker: an objective's
    /// transitive `after` closure inside its quest, and the `quest-complete`
    /// trigger chain between quests. Two writes in one bundle are already
    /// ordered by the bundle's own effect list, so a beat precedes itself here.
    ///
    /// An empty `applied` is forced vacuously — the datum still holds its
    /// declared `initial`, and no play order changes that.
    fn forced_before(
        &self,
        applied: &[StateWriteRecord],
        objective: &str,
        ancestors: &BTreeMap<String, BTreeSet<String>>,
    ) -> bool {
        let gate = Beat::Objective(objective.to_string());
        let mut prev: Option<&Beat> = None;
        for w in applied {
            if let Some(p) = prev
                && !self.beat_precedes(p, &w.beat, ancestors)
            {
                return false;
            }
            prev = Some(&w.beat);
        }
        match prev {
            None => true,
            Some(last) => self.beat_precedes(last, &gate, ancestors),
        }
    }

    /// Does beat `a` necessarily happen at or before beat `b`, in every legal
    /// play order? A beat precedes itself: one bundle's effect list is ordered.
    fn beat_precedes(
        &self,
        a: &Beat,
        b: &Beat,
        ancestors: &BTreeMap<String, BTreeSet<String>>,
    ) -> bool {
        if a == b {
            return true;
        }
        let ancestor = |p: &str, q: &str| {
            ancestors
                .get(q)
                .is_some_and(|set| set.contains(p) && p != q)
        };
        match (a, b) {
            (Beat::Objective(x), Beat::Objective(y)) => {
                let (Some(qx), Some(qy)) = (self.objective_quest(x), self.objective_quest(y))
                else {
                    return false;
                };
                if qx == qy {
                    self.completes_before_armed(x, y)
                } else {
                    ancestor(qx, qy)
                }
            }
            // A quest's `on_complete` runs once every objective of that quest is
            // done, so every objective of it precedes it.
            (Beat::Objective(x), Beat::QuestComplete(q)) => self
                .objective_quest(x)
                .is_some_and(|qx| qx == q || ancestor(qx, q)),
            // …and it runs before any beat of a quest it triggers. Never before a
            // beat of its OWN quest.
            (Beat::QuestComplete(q), Beat::Objective(y)) => {
                self.objective_quest(y).is_some_and(|qy| ancestor(q, qy))
            }
            (Beat::QuestComplete(p), Beat::QuestComplete(q)) => ancestor(p, q),
            // A trigger is a party act the quest DAG does not order: nothing
            // provably precedes or follows it.
            (Beat::Trigger(_), _) | (_, Beat::Trigger(_)) => false,
        }
    }

    /// Per quest, every quest that must complete before it activates — the
    /// transitive closure of the `quest-complete` trigger chain.
    fn quest_ancestors(&self) -> BTreeMap<String, BTreeSet<String>> {
        let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
        for q in &self.c.quests.content.quests {
            if let Trigger::QuestComplete { quest } = &q.trigger {
                parent.insert(q.id.as_str(), quest.as_str());
            }
        }
        let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for q in &self.c.quests.content.quests {
            let mut set = BTreeSet::new();
            let mut cur = q.id.as_str();
            // A cycle here is `DW0130`'s to refuse; bound the walk so this cannot
            // hang on a campaign that carries one.
            for _ in 0..self.c.quests.content.quests.len() {
                let Some(p) = parent.get(cur) else { break };
                if !set.insert((*p).to_string()) {
                    break;
                }
                cur = p;
            }
            out.insert(q.id.as_str().to_string(), set);
        }
        out
    }

    /// The index of the world [`Self::playthrough`] exported, when one completes
    /// the finale.
    pub fn playthrough_world(&self) -> Option<usize> {
        let finale = self.c.quest_plan.content.finale.as_str();
        self.worlds
            .iter()
            .position(|w| self.solve(w).completed.contains(finale))
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
                .any(|k| self.option_takeable_now(npc.as_str(), k, st));
            if !takeable {
                return false;
            }
        }
        true
    }

    // -- internals ---------------------------------------------------------

    /// The replay's starting state: campaign-start quests active, ambient
    /// (trigger / trap payload / trap-disarm) flags saturated.
    fn initial_state(&self) -> ReplayState {
        self.initial_state_with(&|_| true)
    }

    /// [`Self::initial_state`], crediting only the ambient producers `allow`
    /// admits.
    fn initial_state_with(&self, allow: &dyn Fn(&GatedFlag) -> bool) -> ReplayState {
        let mut st = ReplayState::default();
        for q in &self.c.quests.content.quests {
            if matches!(q.trigger, Trigger::CampaignStart) {
                st.active.insert(q.id.as_str().to_string());
            }
        }
        for (id, v) in &self.initial {
            let d = if self.undatable.contains(id) {
                Datum::Undatable
            } else {
                Datum::Known(*v)
            };
            st.state.insert(id.clone(), d);
        }
        self.saturate_ambient_with(&mut st.flags, allow);
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
        self.advance_with(st, step, pos, complete_at, &|_| true);
    }

    /// [`Self::advance`], crediting only the ambient producers `allow` admits.
    fn advance_with(
        &self,
        st: &mut ReplayState,
        step: &PathStep,
        pos: usize,
        complete_at: &mut Option<(usize, String)>,
        allow: &dyn Fn(&GatedFlag) -> bool,
    ) {
        st.done_obj.insert(step.objective.clone());
        st.fired.clear();
        st.undecided.clear();
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
            let beat = Beat::Objective(step.objective.clone());
            let base = format!(
                "/content/quests/{}/on_objective_complete/{}",
                self.quest_index(&step.quest),
                step.objective
            );
            self.fire(effs, &base, st, complete_at, pos, &step.objective, &beat);
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
                let beat = Beat::QuestComplete(qid.to_string());
                let base = format!("/content/quests/{}/on_complete", self.quest_index(qid));
                self.fire(
                    &q.on_complete,
                    &base,
                    st,
                    complete_at,
                    pos,
                    &step.objective,
                    &beat,
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
        self.saturate_ambient_with(&mut st.flags, allow);
    }

    /// The objective with this id, anywhere in the campaign.
    fn objective(&self, id: &str) -> Option<&'a Objective> {
        self.c
            .quests
            .content
            .quests
            .iter()
            .flat_map(|q| &q.objectives)
            .find(|o| o.id().as_str() == id)
    }

    /// The quest that owns `id`.
    fn objective_quest(&self, id: &str) -> Option<&'a str> {
        self.c
            .quests
            .content
            .quests
            .iter()
            .find(|q| q.objectives.iter().any(|o| o.id().as_str() == id))
            .map(|q| q.id.as_str())
    }

    /// Whether objective `earlier` has completed, in every legal play order, before
    /// objective `later` can activate: both are declared by one quest and
    /// `earlier` is in `later`'s transitive `after` closure. `emit::quest::pending_guard`
    /// holds `later` until every `after` objective's score is set, and a completed
    /// objective's score is never cleared, so the relation is the emitter's own.
    ///
    /// Across quests it answers `false` — the quest-chain order is
    /// [`Self::beat_precedes`]'s to add, from the ancestor map it is handed.
    pub(crate) fn completes_before_armed(&self, earlier: &str, later: &str) -> bool {
        match (self.objective_quest(earlier), self.objective_quest(later)) {
            (Some(qe), Some(ql)) if qe == ql => self.after_closure(later).contains(earlier),
            _ => false,
        }
    }

    /// The transitive `after` closure of `id` — every beat the DSL declares must
    /// precede it.
    fn after_closure(&self, id: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![id.to_string()];
        while let Some(cur) = stack.pop() {
            let Some(o) = self.objective(&cur) else {
                continue;
            };
            for a in o.after() {
                if seen.insert(a.as_str().to_string()) {
                    stack.push(a.as_str().to_string());
                }
            }
        }
        seen
    }

    /// The `(npc, node)` the dialogue option that takes `step` opens next, when
    /// the step is a `talk-to` taken by an option with a `next`.
    fn talk_next(&self, step: &PathStep) -> Option<(String, String)> {
        let n = step.talk_option?;
        let npc = match self.objective(&step.objective) {
            Some(Objective::TalkTo { npc, .. }) => npc.as_str(),
            _ => return None,
        };
        let t = self.trees.iter().find(|t| t.npc == npc)?;
        let next = t.options.iter().find(|o| o.n == n)?.next.clone()?;
        Some((npc.to_string(), next))
    }

    /// Whether the emitted dialog draws option `o` in state `st` — the model of
    /// `emit::dialogue::option_display_conditions`: every flag it requires set,
    /// none it forbids set, and every objective it completes pending under the
    /// objective's whole guard (`emit::quest::pending_guard`, spec-0093 §6.3):
    /// its quest running, every `after` complete, its gate's flags holding, and
    /// not yet done. A numeric term is read as open, so the model never hides an
    /// option the datapack might draw.
    fn option_drawn(&self, o: &OptModel, st: &ReplayState) -> bool {
        o.requires.iter().all(|f| st.flags.contains(f))
            && !o.forbids.iter().any(|f| st.flags.contains(f))
            && o.completes.iter().all(|obj| {
                !st.done_obj.contains(obj)
                    && self
                        .objective_quest(obj)
                        .is_some_and(|q| st.active.contains(q))
                    && self.objective(obj).is_none_or(|ob| {
                        ob.after().iter().all(|a| st.done_obj.contains(a.as_str()))
                            && ob
                                .requires_flags()
                                .iter()
                                .all(|f| st.flags.contains(f.as_str()))
                            && !ob
                                .forbids_flags()
                                .iter()
                                .any(|f| st.flags.contains(f.as_str()))
                    })
            })
    }

    /// [`Walk::dialogue_on_screen`] at state `st`: per NPC, the live scene root
    /// ([`Self::scene_root`]), every node reachable from it through drawn
    /// options, and the options drawn on those nodes.
    fn dialogue_on_screen(
        &self,
        casts: &BTreeMap<String, crate::compiler::cast::NpcCast>,
        st: &ReplayState,
        opened: Option<(&str, &str)>,
    ) -> DialogueOnScreen {
        let mut out = DialogueOnScreen::default();
        for t in &self.trees {
            let drawn = |o: &OptModel| self.option_drawn(o, st);
            let mut reach = match self.scene_root(casts, &t.npc, t, st) {
                Some(root) => reachable_from(t, &root, &drawn),
                None => BTreeSet::new(),
            };
            if let Some((npc, node)) = opened
                && npc == t.npc
            {
                reach.extend(reachable_from(t, node, &drawn));
            }
            let mut within: BTreeMap<usize, usize> = BTreeMap::new();
            for o in &t.options {
                let i = within.entry(o.node).or_insert(0);
                let at = *i;
                *i += 1;
                if reach.contains(&o.node) && drawn(o) {
                    out.options
                        .insert((t.npc.clone(), t.node_ids[o.node].clone(), at));
                }
            }
            for ni in reach {
                out.nodes.insert((t.npc.clone(), t.node_ids[ni].clone()));
            }
        }
        out
    }

    /// The dialogue node an NPC's right-click opens in state `st`: the last cast
    /// clause whose quest has begun and whose branch gate holds wins (the emitter's
    /// `dw.cast` dispatch), falling back to the stage-6 root when no clause
    /// governs. `None` when the governing scene is a bark pool or silence — a body
    /// with no options to press.
    fn scene_root(
        &self,
        casts: &BTreeMap<String, crate::compiler::cast::NpcCast>,
        npc: &str,
        t: &TreeModel,
        st: &ReplayState,
    ) -> Option<String> {
        let Some(cast) = casts.get(npc) else {
            return Some(t.root.clone());
        };
        let mut scene: Option<u32> = None;
        for cl in &cast.by_quest {
            if !st.active.contains(&cl.quest)
                || !cl.requires_flags.iter().all(|f| st.flags.contains(f))
                || cl.forbids_flags.iter().any(|f| st.flags.contains(f))
            {
                continue;
            }
            scene = Some(cl.scene);
        }
        match scene {
            None => Some(t.root.clone()),
            Some(i) => match cast.scenes.iter().find(|s| s.index == i).map(|s| &s.action) {
                Some(crate::compiler::cast::SceneAction::Root(r)) => Some(r.clone()),
                _ => None,
            },
        }
    }

    /// The position of quest `id` in the quests document — the index its JSON
    /// pointers carry.
    fn quest_index(&self, id: &str) -> usize {
        self.c
            .quests
            .content
            .quests
            .iter()
            .position(|q| q.id.as_str() == id)
            .unwrap_or(usize::MAX)
    }

    fn quest(&self, id: &str) -> Option<&'a delvewright_dsl::Quest> {
        self.c
            .quests
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == id)
    }

    /// **Every root a body of this tree's NPC can be put in front of**, given
    /// the quests that have begun and the flags that hold: the tree's declared
    /// stage-6 `root`, plus every `cast` ledger root whose quest is active and
    /// whose branch gate holds.
    ///
    /// The single authority for seeding a reachability walk over a tree, and the
    /// reason the model can see a scene only the ledger opens. `scene_root` asks
    /// a narrower question — which ONE root a right-click opens *at this instant*
    /// — because it models the emitted `dw.cast` dispatch, where later
    /// declaration wins. Reachability is the union over the whole playthrough:
    /// a node the ledger opens at any point is a node the party can stand in
    /// front of, whether or not some later clause replaces it.
    ///
    /// The quantifier is per world. A cast clause carrying `requires_flags` is a
    /// per-branch clause, so its root opens the node **in the worlds that hold
    /// those flags and no others** — which is exactly the granularity `DW0482`
    /// asks its question at. `forbids_flags` is ignored for the same reason the
    /// option walk ignores it: this model is monotone, and a negative gate that
    /// closes later cannot un-reach a node the party already stood in.
    ///
    /// `"unchanged"` needs no resolution here: it carries forward a root some
    /// earlier quest already declared, which is already in this union.
    fn entry_roots(
        &self,
        t: &TreeModel,
        active: &BTreeSet<String>,
        flags: &BTreeSet<String>,
    ) -> Vec<String> {
        let mut roots = vec![t.root.clone()];
        for cr in self.cast_roots.get(&t.npc).into_iter().flatten() {
            if !active.contains(&cr.quest) {
                continue;
            }
            if !cr.requires.iter().all(|f| flags.contains(f)) {
                continue;
            }
            if !roots.contains(&cr.root) {
                roots.push(cr.root.clone());
            }
        }
        roots
    }

    /// [`reachable_from`] over every entry point [`Self::entry_roots`] names.
    fn reachable_entered(
        &self,
        t: &TreeModel,
        active: &BTreeSet<String>,
        flags: &BTreeSet<String>,
        takeable: &dyn Fn(&OptModel) -> bool,
    ) -> BTreeSet<usize> {
        let mut seen = BTreeSet::new();
        for root in self.entry_roots(t, active, flags) {
            seen.extend(reachable_from(t, &root, takeable));
        }
        seen
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

    /// Is option `n` of `npc` reachable from one of the roots the campaign can
    /// open at this point, through options whose own gates `flags` satisfies?
    /// (Branch selection is not applied here: the replay already committed to
    /// one option per `talk-to`.)
    fn option_takeable_now(&self, npc: &str, n: usize, st: &ReplayState) -> bool {
        let Some(t) = self.trees.iter().find(|t| t.npc == npc) else {
            return false;
        };
        let flags = &st.flags;
        let reach = self.reachable_entered(t, &st.active, flags, &|o: &OptModel| {
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

    /// Add every ambient (trigger / trap payload / trap-disarm) flag whose gate
    /// is satisfied, to fixpoint.
    /// [`Self::saturate_ambient`] over the producers `allow` admits.
    fn saturate_ambient_with(
        &self,
        flags: &mut BTreeSet<String>,
        allow: &dyn Fn(&GatedFlag) -> bool,
    ) {
        loop {
            let mut changed = false;
            for g in &self.ambient {
                if allow(g)
                    && g.requires.iter().all(|f| flags.contains(f))
                    && flags.insert(g.flag.clone())
                {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Fire an effect bundle at a concrete point in the replay: honor every gate
    /// against the current state, descend into `sequence` steps and `on_arrive`
    /// bundles, and skip `on_respawn`/`on_caught` reaction bundles.
    ///
    /// **The gate honored here is the whole gate**, not two of its three fields.
    /// Sibling effects are consecutive commands in one generated function and
    /// vanilla evaluates each `execute` condition where it stands, so a write's
    /// own `requires_state` is read against the value the bundle holds *at that
    /// effect* — which is what makes the value this walk carries the value the
    /// datapack produces. A numeric term the walk cannot decide (the datum is
    /// [`Datum::Undatable`]) is treated as OPEN: withholding a producer would be
    /// the unsound direction, and it is the same conservative stance the flag
    /// half already takes for `forbids_flags`.
    #[allow(clippy::too_many_arguments)]
    fn fire(
        &self,
        effs: &[QuestEffect],
        base: &str,
        st: &mut ReplayState,
        complete_at: &mut Option<(usize, String)>,
        pos: usize,
        objective: &str,
        beat: &Beat,
    ) {
        // **A value this list pinned, for the lines after it in the same list.**
        // The lines of one list are consecutive commands of one function, which
        // vanilla runs in one tick: no tick boundary falls between them, so no
        // undated write — a death, a purchase, a strike — can land between a
        // `set-state` and the gate that reads it a line later. A datum the walk
        // cannot date is therefore known, inside this list only, from the line
        // that pins it. Never inherited by a nested list (a `sequence` step or an
        // `on_arrive` runs at a later tick) and never carried past the list.
        let mut pinned: BTreeMap<String, i64> = BTreeMap::new();
        let value = |st: &ReplayState, pinned: &BTreeMap<String, i64>, id: &str| match st
            .state
            .get(id)
            .copied()
        {
            Some(Datum::Undatable) => pinned
                .get(id)
                .map_or(Datum::Undatable, |v| Datum::Known(*v)),
            other => other.unwrap_or(Datum::Undatable),
        };
        for (i, e) in effs.iter().enumerate() {
            let gated = !e
                .requires_flags()
                .iter()
                .all(|f| st.flags.contains(f.as_str()))
                || e.forbids_flags()
                    .iter()
                    .any(|f| st.flags.contains(f.as_str()));
            // A numeric term this walk can decide and that does not hold closes
            // the effect exactly as an unset flag does. `Some(false)` is the only
            // closing answer: `None` is undatable, and an undatable gate has not
            // been shown to close.
            let numerically_closed = e
                .requires_state()
                .iter()
                .any(|cmp| value(st, &pinned, cmp.state.as_str()).satisfies(cmp) == Some(false));
            if gated || numerically_closed {
                // A write this walk did NOT perform is what makes a later gate's
                // value depend on more than the order — record it so a refusal
                // downstream can withhold rather than guess.
                if let Some((id, _)) = e.writes_state() {
                    st.unapplied.insert(id.as_str().to_string());
                }
                continue;
            }
            let path = format!("{base}/{i}");
            // Open, but only because a term could not be dated: the walk goes
            // on through it, and the line is recorded as not decided.
            let undatable = e.requires_state().iter().any(|cmp| {
                value(st, &pinned, cmp.state.as_str())
                    .satisfies(cmp)
                    .is_none()
            });
            if undatable {
                st.undecided.push(path.clone());
            }
            st.fired.push(path.clone());
            match &e.verb {
                Verb::SetFlag { flag, .. } => {
                    st.flags.insert(flag.as_str().to_string());
                }
                Verb::CampaignComplete { .. } => {
                    if complete_at.is_none() {
                        *complete_at = Some((pos, objective.to_string()));
                    }
                }
                Verb::SetCheckpoint { .. } | Verb::Bonfire { .. } | Verb::BeginStealth { .. } => {
                    continue;
                }
                _ => {}
            }
            if let Some((id, w)) = e.writes_state() {
                let id = id.as_str().to_string();
                let initial = self.initial.get(&id).copied().unwrap_or(0);
                let before = st.state.get(&id).copied().unwrap_or(Datum::Undatable);
                let after = before.write(w, initial);
                if after == Datum::Undatable {
                    let pin = match w {
                        StateWrite::Set(v) => Some(i64::from(v)),
                        StateWrite::Clear => Some(initial),
                        StateWrite::Add(v) => pinned.get(&id).map(|p| p + i64::from(v)),
                    };
                    match pin {
                        Some(v) => {
                            pinned.insert(id.clone(), v);
                        }
                        None => {
                            pinned.remove(&id);
                        }
                    }
                }
                st.state.insert(id.clone(), after);
                st.wrote.entry(id).or_default().push(StateWriteRecord {
                    beat: beat.clone(),
                    position: pos,
                    verb: e.verb.tag(),
                    after,
                });
            }
            for (pseg, _, list) in e.nested_effect_lists_labeled() {
                let nested = format!("{path}/{pseg}");
                self.fire(list, &nested, st, complete_at, pos, objective, beat);
            }
        }
    }

    /// The monotone fixpoint, restricted to one branch world.
    fn solve(&self, world: &World) -> Solution {
        let mut s = Solution::default();
        loop {
            let mut changed = false;
            self.saturate_ambient_tracked(&mut s.flags, &mut changed);

            // Dialogue: options reachable from any root this world can put a body
            // in front of — the tree's own and every live `cast` ledger scene —
            // through takeable options, restricted to this world's selected
            // alternatives.
            let mut dlg_completes: BTreeMap<String, usize> = BTreeMap::new();
            let snapshot = s.flags.clone();
            let active = s.active.clone();
            for t in &self.trees {
                let takeable = |o: &OptModel| {
                    o.requires.iter().all(|f| snapshot.contains(f))
                        && self.selectable(&t.npc, o.n, world)
                };
                let reach = self.reachable_entered(t, &active, &snapshot, &takeable);
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
    collect_flags_from(effs, gate, None, out);
}

/// [`collect_flags`], each producer stamped with the trigger that fires it.
fn collect_flags_from(
    effs: &[QuestEffect],
    gate: &[String],
    trigger: Option<&str>,
    out: &mut Vec<GatedFlag>,
) {
    for e in effs {
        let mut here: Vec<String> = gate.to_vec();
        here.extend(e.requires_flags().iter().map(|f| f.as_str().to_string()));
        here.sort();
        here.dedup();
        if let Verb::SetFlag { flag, .. } = &e.verb {
            out.push(GatedFlag {
                flag: flag.as_str().to_string(),
                requires: here.clone(),
                trigger: trigger.map(str::to_string),
            });
        }
        match &e.verb {
            Verb::SetCheckpoint { .. } | Verb::Bonfire { .. } | Verb::BeginStealth { .. } => {
                continue;
            }
            _ => {
                for list in e.nested_effect_lists() {
                    collect_flags_from(list, &here, trigger, out);
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

/// Every `cast` ledger dialogue root the campaign declares, by NPC.
///
/// The ledger is the second entry point into a dialogue tree, and the DSL has
/// held that since spec-0020: `validate` seeds `DW0120`'s orphan walk from the
/// same set (`NpcDialogue::reachable_from`, "always relative to a root SET").
/// This is that set, in the flow model's own shape, so the two halves of the
/// engine agree about what a body can be shown.
fn cast_roots(c: &Campaign) -> BTreeMap<String, Vec<CastRoot>> {
    let mut out: BTreeMap<String, Vec<CastRoot>> = BTreeMap::new();
    for q in &c.quests.content.quests {
        for (npc, entry) in &q.cast {
            for p in entry.placements() {
                let Some(delvewright_dsl::CastDialogue::Root(r)) = &p.dialogue else {
                    continue;
                };
                out.entry(npc.as_str().to_string())
                    .or_default()
                    .push(CastRoot {
                        quest: q.id.as_str().to_string(),
                        root: r.as_str().to_string(),
                        requires: p.requires_flags.iter().map(|f| f.as_str().into()).collect(),
                    });
            }
        }
    }
    out
}

/// Node indices reachable from `root` (a node id of `t`), traversing only
/// options `takeable` admits. The live [`crate::compiler::cast`] scene decides which root
/// a right-click actually opens, which is why this is parameterized.
fn reachable_from(
    t: &TreeModel,
    root: &str,
    takeable: &dyn Fn(&OptModel) -> bool,
) -> BTreeSet<usize> {
    let mut seen = BTreeSet::new();
    let Some(&root) = t.index.get(root) else {
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

/// **The data no ordered walk of this campaign can date**, and therefore the
/// data no comparison is ever refused against.
///
/// A flag is monotone and party-wide, so an ambient producer the party may fire
/// at any moment is credited unconditionally — the flag is either set or it is
/// not, and firing again changes nothing. A datum is neither: firing an ambient
/// `add-state` twice is a different number, and `at-least`/`at-most` read that
/// number. So the three ways a value stops being a function of the path are
/// named here, and each yields the whole datum rather than a moment of it:
///
/// 1. **An ambient root writes it** — an environment trigger, a trap payload, a
///    shortcut's `on_unlock`, a shop offer's effects. The party can walk over
///    and fire any of those, any number of times, at any point. (The shop case
///    is the one already stated in [`Flow::new`]'s producer table: a price is a
///    runtime balance no static model can date.)
/// 2. **A reaction bundle writes it** — the campaign's `on_death`, a dialogue
///    or quest `set-checkpoint`'s `on_respawn`, a `bonfire`'s `on_rest`, a
///    `begin-stealth`'s `on_caught`. Those fire when somebody dies, rests or is
///    seen, which is exactly the moment this model refuses to name — the stance
///    [`Flow::fire`] already takes by not descending into them.
/// 3. **A stake forfeits it** — `stakes[].state`. The forfeit is a field rather
///    than an effect, so no effect walk reaches it, and a death moves the number
///    by a proportion of whatever the purse held.
///
/// A fourth case is about the walk rather than the campaign: a `player`-scoped
/// datum under `min_players >= 2`. The replay is one agent's walk, and which
/// agent performs which arm of a division of labour is nothing the document
/// says, so "the acting player's value at this beat" is not a function of the
/// path either. A `party`-scoped datum has one holder and is unaffected.
fn undatable_state(c: &Campaign) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    crate::compiler::plan::for_each_effect_root(c, &mut |site, effs| {
        let dated = matches!(
            site.root,
            crate::compiler::plan::EffectRoot::ObjectiveComplete { .. }
                | crate::compiler::plan::EffectRoot::QuestComplete(_)
        );
        collect_undated_writes(effs, !dated, &mut out);
    });
    for s in &c.quests.content.stakes {
        out.insert(s.state.as_str().to_string());
    }
    // A loop's `counts` (spec-0086 §3.4) moves on every crossing any body
    // makes, which no ordered walk dates; the loop replay in `plan` reads it
    // on its own terms.
    for l in &c.quests.content.loops {
        if let Some(counts) = &l.counts {
            out.insert(counts.as_str().to_string());
        }
    }
    if crate::compiler::plan::min_players(c) >= 2 {
        for s in &c.quests.content.state {
            if s.scope == StateScope::Player {
                out.insert(s.id.as_str().to_string());
            }
        }
    }
    // A datum nothing declares has no `initial` to start from and no scope to
    // read it at; `DW0500` owns that, and this walk carries no opinion.
    out.retain(|id| c.quests.content.state_decl(id).is_some());
    out
}

/// The writes in `effs` that no replay dates: all of them when `all`, otherwise
/// only those inside a reaction bundle the replay does not fire.
///
/// The descent mirrors [`Flow::fire`] exactly — `sequence` steps and `on_arrive`
/// bundles are replayed, `on_respawn` / `on_rest` / `on_caught` are not — so the
/// two cannot disagree about which writes an ordered walk performs.
fn collect_undated_writes(effs: &[QuestEffect], all: bool, out: &mut BTreeSet<String>) {
    for e in effs {
        let reaction = matches!(
            &e.verb,
            Verb::SetCheckpoint { .. } | Verb::Bonfire { .. } | Verb::BeginStealth { .. }
        );
        if all && let Some((id, _)) = e.writes_state() {
            out.insert(id.as_str().to_string());
        }
        for list in e.nested_effect_lists() {
            collect_undated_writes(list, all || reaction, out);
        }
    }
}

/// Every flag read by a gate anywhere in the campaign: the `requires_flags` /
/// `forbids_flags` of every objective, dialogue option, environment trigger and
/// trap, plus those of **every effect the compiler can lower**, at every nesting
/// depth.
///
/// A choice group none of whose flags is read cannot change any reachability
/// verdict, so it never participates in world enumeration
/// ([`enumerate_worlds`]) — which makes this an inventory that must be
/// **complete or the model is imprecise**: a group whose flags are read only at a
/// root this misses stays unconstrained, and one world then holds two mutually
/// exclusive branch flags at once (a false green, in the union direction).
///
/// The effect half therefore walks [`crate::compiler::plan::for_each_effect_root`], the one
/// enumeration of the five roots emission lowers from, rather than a second
/// hand-maintained list of three. Unlike the producer model above
/// this needs no per-root policy: whether a firing is guaranteed does not change
/// whether its gate reads a flag, and the compiler emits that gate at all five
/// roots alike.
pub fn gate_flags(c: &Campaign) -> BTreeSet<String> {
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
    }
    for t in &c.quests.content.triggers {
        eat(&t.requires_flags, &mut out);
        eat(&t.forbids_flags, &mut out);
    }
    for t in &c.quests.content.traps {
        eat(&t.requires_flags, &mut out);
        eat(&t.forbids_flags, &mut out);
    }
    // A loop's gate is its release (spec-0086): a branch flag only a loop reads
    // still decides which world the party walks.
    for l in &c.quests.content.loops {
        eat(&l.requires_flags, &mut out);
        eat(&l.forbids_flags, &mut out);
    }
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for o in &node.options {
                eat(&o.requires_flags, &mut out);
                eat(&o.forbids_flags, &mut out);
            }
        }
    }
    crate::compiler::plan::for_each_effect_root(c, &mut |_site, effs| {
        for e in effs {
            e.visit_deep(&mut |x| {
                for f in x.requires_flags().iter().chain(x.forbids_flags()) {
                    out.insert(f.as_str().to_string());
                }
            });
        }
    });
    // spec-0088: a lethal volume live from a story stage reads its flags too.
    for v in &c.quests.content.lethal_volumes {
        let g = v.gate();
        eat(g.requires_flags, &mut out);
        eat(g.forbids_flags, &mut out);
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

/// Topologically order **every** quest in `keep` by `depends_on` (Kahn), rather
/// than only the finale's dependency closure — the branch playthrough's ordering
/// (spec-0025). Same algorithm as [`finale_order`], different root set.
fn dag_order(c: &Campaign, keep: &BTreeSet<String>) -> (Vec<String>, bool) {
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
    let needed: BTreeSet<&str> = plan
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .filter(|q| keep.contains(*q))
        .collect();
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
    // A quest that can end the delve is walked as late as its dependants allow:
    // `campaign-complete` belongs to the path's last step, so every other quest
    // this world completes (an optional strand included) is walked before it.
    // Without this the tie between a ready ending and a ready strand fell to the
    // id order, and a strand whose id sorts after the ending was exported past
    // the end of the delve, where no player can walk it.
    let ending: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .filter(|q| quest_can_end_the_delve(q))
        .map(|q| q.id.as_str())
        .collect();
    let mut ready: VecDeque<&str> = VecDeque::new();
    let mut held: VecDeque<&str> = VecDeque::new();
    for (q, _) in indeg.iter().filter(|&(_, &d)| d == 0) {
        if ending.contains(q) {
            held.push_back(q);
        } else {
            ready.push_back(q);
        }
    }
    let mut order = Vec::new();
    while let Some(q) = ready.pop_front().or_else(|| held.pop_front()) {
        order.push(q.to_string());
        for r in &needed {
            if deps.get(r).is_some_and(|ds| ds.contains(&q)) {
                let e = indeg.get_mut(*r).unwrap();
                *e -= 1;
                if *e == 0 {
                    if ending.contains(r) {
                        held.push_back(r);
                    } else {
                        ready.push_back(r);
                    }
                }
            }
        }
    }
    let cyclic = order.len() != needed.len();
    (order, cyclic)
}

/// Whether any effect list `q` declares (its own `on_complete`, any objective's
/// `on_objective_complete`, and every list nested inside them) carries a
/// `campaign-complete`. Syntactic: a gated ending still counts, which only
/// moves a quest later in a branch walk.
fn quest_can_end_the_delve(q: &delvewright_dsl::Quest) -> bool {
    fn ends(effs: &[QuestEffect]) -> bool {
        effs.iter().any(|e| {
            matches!(e.verb, Verb::CampaignComplete { .. })
                || e.nested_effect_lists().into_iter().any(ends)
        })
    }
    ends(&q.on_complete) || q.on_objective_complete.values().any(|l| ends(l))
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
