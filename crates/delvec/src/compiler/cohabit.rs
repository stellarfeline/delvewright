//! **One mark, one body** — two bodies whose lifetimes overlap may not be
//! declared on the same cell (`DW0896`).
//!
//! ## The defect this exists for
//!
//! A castle-tour campaign closes on a muster: a captain of the guard and six
//! men-at-arms come out under the gate arch. All seven are stage-5 actors, and
//! all seven declare `anchor/stop-gate`. The built datapack says so in one line,
//! seven times:
//!
//! ```text
//! summon minecraft:mannequin 77.5 68.0 40.5
//! ```
//!
//! One coordinate triple for seven bodies. The build exited 0. What keeps the
//! guard from being a pile of mannequins in one block is that each one is walked
//! off the mark by a `move-actor` at a slightly different speed — so the rank the
//! author wrote comes out as single file, and the only thing separating the
//! bodies at all is how fast they happen to leave.
//!
//! ## The rule, and the one it is NOT
//!
//! The obvious rule — *two bodies on one anchor is an error* — is wrong, and the
//! gallery is the proof. `actor/sergeant` and `npc/marshal` both stand on
//! `anchor/muster`, deliberately: the engine supports a **handoff**, where one
//! body leaves a mark and another takes its place there. A rule keyed to a shared
//! anchor either refuses that or gets widened until it binds to nothing.
//!
//! The property that is a defect is **co-existence**: two bodies that are in the
//! world at the same time, declared on one cell. The handoff is not
//! co-existence — the bodies take turns. The muster is: the captain is standing
//! under the arch when the first man-at-arms is summoned onto the captain's own
//! mark.
//!
//! The rule is about the **declaration**, not about a tick. It does not ask
//! whether two bodies are inside each other at some instant — a body that has
//! walked off its mark is not on it any more, and answering that question would
//! make the verdict depend on `move-actor` speeds, which is precisely the thing
//! the muster was relying on. It asks whether the campaign gives two
//! simultaneously-living bodies one cell to be summoned onto. It does, or it does
//! not, and no walk changes the answer.
//!
//! ## What the compiler can prove
//!
//! [`crate::compiler::timeline`]'s stance, unchanged: **state only what the
//! structure proves**, and let every uncertainty withhold the error rather than
//! invent one. Two arms, and nothing else counts:
//!
//! * **World init.** Every non-`deferred` npc stands on its mark from world
//!   init, unconditionally and simultaneously. Two of them on one cell are in
//!   one cell at tick 0, and no effect anywhere can make that untrue, so this
//!   arm needs no timeline at all.
//! * **A body still standing when another is summoned.** A body is *provably
//!   live* at an effect when (a) nothing in the campaign can remove it, and
//!   (b) it is either a world-init body or was entered by an unconditional
//!   `spawn-npc` / `spawn-actor` earlier in that effect's **own** timeline.
//!   Cross-bundle order is unknowable and is never guessed.
//!
//! **Nothing can remove it** is [`delvewright_dsl::QuestEffect::body_exit`] over
//! every effect at every depth — a `despawn-npc`, a `despawn-actor`, or an
//! `unleash-actor`, after which the body is a real-AI twin the compiler makes no
//! claim about — plus, for an actor, `vulnerable: true`, because a body a player
//! can kill has a lifetime the compiler cannot bound. So the live set holds only
//! bodies whose liveness, once established, cannot be revoked, and the replay
//! never has to model an exit: a body with an exit never enters the set.
//!
//! The two directions are deliberately asymmetric, and the asymmetry is the
//! conservative one in both halves:
//!
//! * To put a body **into** the live set its entry must be unconditional — a
//!   `spawn-actor` carrying `requires_flags` may not fire, so it proves nothing.
//! * To **judge** an entry its own condition is irrelevant. A conditional
//!   `spawn-npc` onto a cell a live body holds collides on every run where it
//!   fires; that it sometimes does not fire is not a defence.
//!
//! ## No escape hatch, and why there is none
//!
//! There is no legitimate deliberate overlap to exempt. Two entity bodies at one
//! coordinate interpenetrate: a puppet is `NoAI`/`immovable`, so it does not even
//! push its neighbour away — it simply stands inside it. The DSL expresses no
//! mount, no rider and no stacked body, so nothing an author can write wants
//! this. An opt-out would therefore have to be secured by the author's say-so,
//! which is exactly the property a mistake also supplies — the anti-pattern
//! CLAUDE.md names in those words. The remedy is a second mark, and a campaign
//! that wants two bodies side by side needs two anchors anyway, since one anchor
//! is one cell.
//!
//! ## What this deliberately does not catch
//!
//! * **Bodies one cell apart whose hitboxes still meet.** The rule is the cell,
//!   not the box. A cell is what an anchor resolves to and what a `summon`
//!   names; a box rule would need a margin, and a margin is a threshold to
//!   argue about. `DW0450` ([`crate::compiler::clearance`]) is the box rule, and
//!   it judges bodies against geometry.
//! * **Two bodies both of which carry an exit.** The muster's six men-at-arms
//!   each end in a `despawn-actor` inside their own `move-actor`'s `on_arrive`,
//!   and an `on_arrive` is not ordered against the enclosing bundle's later
//!   siblings — so the compiler cannot prove man-at-arms 1 is still there when
//!   man-at-arms 2 is summoned, and does not say it is. The captain, whom
//!   nothing removes, is caught against every one of them.
//! * **A pair whose entries sit in two different bundles.** Cross-bundle order
//!   is unknowable; the DAG-causal models own what the player is forced to walk.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{BodyRef, DwCode, ExitTier, QuestEffect, body_sites};

use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;

/// `DW0896`: **two bodies whose lifetimes overlap are declared on one cell.**
///
/// Error tier, with no authorable exemption. A mark is a cell and a cell holds
/// one body; the repair is a second anchor, and it is always available.
pub const DW_ONE_MARK_TWO_BODIES: DwCode = DwCode::new("DW0896", ExitTier::Build);

/// One body, as this proof reads it: who it is, where it was declared, and the
/// cell it is summoned onto.
#[derive(Clone, Debug)]
struct Placed<'a> {
    /// The body itself — the class is a compile error away from being forgotten.
    body: BodyRef<'a>,
    /// JSON pointer at the declaration ([`body_sites`]'s pointer, at the object).
    path: String,
    /// The cell the anchor resolves to.
    cell: [i32; 3],
}

impl Placed<'_> {
    /// ``actor `actor/captain` (quests /content/actors/0, on `anchor/stop-gate`)``
    fn describe(&self) -> String {
        format!(
            "{} `{}` ({} {}, on `{}`)",
            match self.body {
                BodyRef::Npc(_) => "npc",
                BodyRef::Actor(_) => "actor",
            },
            self.body.id(),
            self.body.stage(),
            self.path,
            self.body.anchor().as_str(),
        )
    }
}

/// What `DW0896` examined, over the campaign's own bodies (CLAUDE.md's vacuity
/// rule: every validation artifact states its binding count, with its
/// denominator).
///
/// The zeroes that can occur, and what each means:
///
/// * `bodies` zero — the campaign declares no npc and no actor. There is nothing
///   to judge and nothing is claimed.
/// * `placed` zero over non-zero `bodies` — every body's anchor is dangling, and
///   `DW0325`/`DW0345`/`DW0360` are the ones saying so. Every verdict below is
///   then vacuous rather than clean.
/// * `pairs` zero over non-zero `placed` — no two bodies were ever both provably
///   live, so nothing was compared. This is the shape a reader has to be able to
///   see: the count is printed on a run that found nothing, or it says nothing at
///   all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CohabitBinding {
    /// The denominator: bodies the campaign declares, of every class.
    pub bodies: usize,
    /// Of those, the ones whose anchor resolves to a cell.
    pub placed: usize,
    /// Distinct cells those bodies resolve to. Equal to `placed` is a campaign
    /// where no two bodies share a mark at all.
    pub cells: usize,
    /// Placed bodies standing on their mark from world init.
    pub at_init: usize,
    /// Placed bodies nothing in the campaign can remove — the ones whose
    /// liveness, once established, is permanent.
    pub permanent: usize,
    /// Entry events judged, at every depth and at **both** entry points — a
    /// quest bundle's `spawn-npc`/`spawn-actor` and a dialogue option's
    /// `spawn-npc`.
    pub entries: usize,
    /// Co-existence questions actually asked: one per (living body, entering
    /// body) comparison, plus one per world-init pair.
    pub pairs: usize,
    /// Distinct pairs refused (`DW0896`).
    pub refused: usize,
}

impl CohabitBinding {
    /// The one line this proof owes its reader.
    pub fn line(&self) -> String {
        format!(
            "one-mark binding: {} body(ies) declared, {} of them resolving to {} distinct cell(s); \
             {} stand there from world init and {} are bodies nothing in this campaign can \
             remove; {} co-existence question(s) asked over {} entry event(s), {} refused \
             (DW0896).",
            self.bodies,
            self.placed,
            self.cells,
            self.at_init,
            self.permanent,
            self.pairs,
            self.entries,
            self.refused
        )
    }
}

/// Every effect the campaign can fire, at every depth, in the canonical
/// pre-order — [`crate::compiler::timeline`]'s replay with no state folded.
///
/// The same walk the gate model and the emitter read, so "which firings exist"
/// is one answer.
fn all_effects<'a>(plan: &Plan<'a>) -> Vec<&'a QuestEffect> {
    let mut out: Vec<(&'a QuestEffect, ())> = Vec::new();
    crate::compiler::plan::for_each_effect_root(plan.campaign, &mut |_, effs| {
        crate::compiler::timeline::replay_list(effs, &(), &mut |_, _| {}, &mut out);
    });
    out.into_iter().map(|(e, ())| e).collect()
}

/// **One mark, one body** (`DW0896`), and what the proof examined.
///
/// The binding is returned beside the verdict rather than printed here, so the
/// caller states it on every run — including the run that refuses, and including
/// the run that found nothing.
pub fn check_one_body_per_mark<'a>(plan: &Plan<'a>) -> (CohabitBinding, Result<(), Failure>) {
    let c = plan.campaign;
    let sites = body_sites(c);
    let mut b = CohabitBinding {
        bodies: sites.len(),
        ..Default::default()
    };

    // Where every body stands, skipping the ones whose anchor resolves to
    // nothing (a dangling reference is another check's finding).
    let placed: Vec<Placed<'a>> = sites
        .iter()
        .filter_map(|s| {
            plan.body_point(s.body).map(|cell| Placed {
                body: s.body,
                path: s.path.clone(),
                cell,
            })
        })
        .collect();
    b.placed = placed.len();
    b.cells = placed.iter().map(|p| p.cell).collect::<BTreeSet<_>>().len();
    if placed.is_empty() {
        return (b, Ok(()));
    }
    let at: BTreeMap<&'a str, &Placed<'a>> = placed.iter().map(|p| (p.body.id(), p)).collect();

    // A body nothing can remove. The exit verbs are `body_exit`'s closed set; a
    // player-killable actor is the fourth way a body ends and is a property of
    // the body rather than of any effect.
    let effects = all_effects(plan);
    let removable: BTreeSet<&'a str> = effects
        .iter()
        .filter_map(|e| e.body_exit())
        .chain(
            placed
                .iter()
                .filter(|p| p.body.killable_by_players())
                .map(|p| p.body.id()),
        )
        .collect();
    let permanent = |id: &str| !removable.contains(id);
    b.at_init = placed.iter().filter(|p| p.body.at_world_init()).count();
    b.permanent = placed.iter().filter(|p| permanent(p.body.id())).count();

    // Refused pairs, normalised so one pair is one finding however many ways the
    // campaign reaches it, and ordered so the message is deterministic.
    let mut refused: BTreeSet<(&'a str, &'a str)> = BTreeSet::new();

    // Arm 1 — world init. Every non-`deferred` npc is summoned at tick 0, all of
    // them at once and none of them conditionally, so two on one cell are in one
    // cell before any effect has fired. No permanence needed: a later
    // `despawn-npc` cannot undo tick 0.
    let init: Vec<&Placed<'a>> = placed.iter().filter(|p| p.body.at_world_init()).collect();
    for (i, x) in init.iter().enumerate() {
        for y in &init[i + 1..] {
            b.pairs += 1;
            if x.cell == y.cell {
                note(&mut refused, x.body.id(), y.body.id());
            }
        }
    }

    // Arm 2 — a body still standing when another is summoned onto its mark.
    //
    // The replayed state is the set of bodies provably live at the effect about
    // to fire, and it holds only bodies whose liveness cannot be revoked — so
    // there is no exit to fold. It starts, in every timeline, from the world-init
    // bodies nothing can remove; nothing a previous bundle did is assumed.
    let live0: BTreeSet<&'a str> = placed
        .iter()
        .filter(|p| p.body.at_world_init() && permanent(p.body.id()))
        .map(|p| p.body.id())
        .collect();
    let mut fold = |e: &'a QuestEffect, live: &mut BTreeSet<&'a str>| {
        // Only an UNCONDITIONAL entry of a permanent body proves anything about
        // what is standing later in this timeline.
        if e.requires_flags().is_empty()
            && e.forbids_flags().is_empty()
            && let Some(id) = e.body_entry()
            && let Some(p) = at.get(id)
            && permanent(id)
        {
            live.insert(p.body.id());
        }
    };
    crate::compiler::plan::for_each_effect_root(c, &mut |_, effs| {
        let mut out: Vec<(&'a QuestEffect, BTreeSet<&'a str>)> = Vec::new();
        crate::compiler::timeline::replay_list(effs, &live0, &mut fold, &mut out);
        for (e, live) in out {
            // Judged whatever the entry's own condition is: an entry that fires
            // onto an occupied mark collides on every run where it fires.
            let Some(id) = e.body_entry() else { continue };
            let Some(entering) = at.get(id) else { continue };
            b.entries += 1;
            for other in &live {
                if *other == id {
                    continue;
                }
                b.pairs += 1;
                if at[other].cell == entering.cell {
                    note(&mut refused, at[other].body.id(), entering.body.id());
                }
            }
        }
    });

    // The other entry point: a body that walks in mid-conversation
    // (`DialogueEffect::spawn-npc`). A dialogue option is not one of the effect
    // roots the timeline replay orders, so nothing here is proven to precede it
    // and its live set is the world-init one; within the option's own list the
    // same asymmetry applies — a gated option proves nothing about what it
    // leaves standing, and an entry is judged whether or not its option is
    // gated. Enumerating it is not optional: a gate that names one of two doors
    // guards neither.
    for d in &c.dialogue.content.dialogues {
        for node in &d.nodes {
            for opt in &node.options {
                let ungated = opt.requires_flags.is_empty()
                    && opt.forbids_flags.is_empty()
                    && opt.requires_state.is_empty();
                let mut live = live0.clone();
                for e in &opt.effects {
                    let Some(id) = e.body_entry() else { continue };
                    let Some(entering) = at.get(id) else { continue };
                    b.entries += 1;
                    for other in &live {
                        if *other == id {
                            continue;
                        }
                        b.pairs += 1;
                        if at[other].cell == entering.cell {
                            note(&mut refused, at[other].body.id(), entering.body.id());
                        }
                    }
                    if ungated && permanent(id) {
                        live.insert(entering.body.id());
                    }
                }
            }
        }
    }

    b.refused = refused.len();
    let Some((first, second)) = refused.iter().next().copied() else {
        return (b, Ok(()));
    };
    let rest: Vec<String> = refused
        .iter()
        .skip(1)
        .map(|(x, y)| format!("`{x}` + `{y}`"))
        .collect();
    (
        b,
        Err(Failure {
            code: DW_ONE_MARK_TWO_BODIES,
            message: message(at[first], at[second], &rest),
        }),
    )
}

/// Record one refused pair, normalised by id so the campaign reaching it twice
/// is still one finding and the message order does not depend on which arm
/// found it.
fn note<'a>(set: &mut BTreeSet<(&'a str, &'a str)>, x: &'a str, y: &'a str) {
    set.insert(if x <= y { (x, y) } else { (y, x) });
}

/// The refusal, naming both bodies, the cell they share, and what it costs.
fn message(x: &Placed<'_>, y: &Placed<'_>, rest: &[String]) -> String {
    let [cx, cy, cz] = x.cell;
    let mut m = format!(
        "{} and {} are both declared on world cell [{cx}, {cy}, {cz}], and both are in the \
         world at the same time. A mark is one cell and one cell holds one body: the delve \
         would summon both of them onto the same coordinate, where they stand inside each \
         other. Give each body its own anchor — one anchor is one cell, so a rank of bodies \
         needs a mark apiece — or make the campaign prove they take turns, by removing the \
         first with a `despawn-npc` / `despawn-actor` before the second is summoned. Bodies \
         that take turns on one mark are the supported handoff and are not refused; what is \
         refused is two live bodies with one place to stand. Nothing else keeps them apart: a \
         `move-actor` walking one of them off the mark separates them only by however fast it \
         happens to leave.",
        x.describe(),
        y.describe(),
    );
    if !rest.is_empty() {
        m.push_str(&format!(
            " {} further pair(s) share a mark the same way: {}.",
            rest.len(),
            rest.join(", ")
        ));
    }
    m
}
