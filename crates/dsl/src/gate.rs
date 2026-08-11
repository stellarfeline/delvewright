//! **The one gate.**
//!
//! A *gate* is the campaign's answer to "may this happen yet?". Six object
//! classes ask it — an objective, an effect, an environment trigger, a trap, a
//! dialogue option and a cast placement — and until DSL v0.10 each of them
//! carried its own copy of the same two fields, `requires_flags` and
//! `forbids_flags`, with nothing in the type system saying they were one thing.
//!
//! That arrangement is the defect CLAUDE.md names first. When spec-0031 needed a
//! numeric comparison ("this door opens at 500", "this line is withheld below
//! 200", "this lever does nothing while the car is moving"), the shape of the
//! code offered exactly one obvious place to put it: the verb that asked. The
//! second consumer would then have had no surface, and the fix would have looked
//! like a second bespoke field. **Generality is decided at the FIRST site.**
//!
//! So `requires_state` was added to all twenty-five declaration sites at once,
//! and this module is what makes that a property rather than a coincidence:
//!
//! * [`Gate`] is the gate as one value — the three fields together, borrowed.
//!   Every consumer answers `gate()`, and every proof that reasons about gating
//!   is written against it rather than against two of three fields.
//! * [`GateConsumer`] is the **closed set** of object classes that carry a gate.
//!   `ALL` is the enumeration; adding a variant is a rustc error at every match.
//! * [`for_each_gate`] visits every gate in a campaign, in one fixed order, and
//!   returns a [`GateBinding`] ledger stating how many gates it found per
//!   consumer — because a proof over "every gate" that bound to nothing is
//!   vacuous, not a pass (CLAUDE.md).
//!
//! What this module cannot do is stop a twenty-sixth *declaration site* being
//! added tomorrow with only the flag pair on it. Nothing in Rust's type system
//! can: the fields are ordinary fields on ordinary structs, and serde's
//! `flatten` — the one construct that would have made a shared struct
//! literal — is a compile error in combination with `deny_unknown_fields`, which
//! every stage struct carries and which is what turns an author's typo into
//! `DW0100` instead of silence. That half of the obligation is
//! `crates/dsl/tests/gate_consumers.rs`, which enumerates the consumers **from
//! the generated JSON Schema** — i.e. from the types — and fails when any schema
//! object declares `requires_flags` without `requires_state`.
//!
//! Determinism (ADR-0006): iteration is over slices and `BTreeMap` keys, in a
//! fixed order that is part of this module's contract.

use crate::envelope::Campaign;
use crate::ids::FlagId;
use crate::stages::StateCompare;

/// A gate, as one value: everything that decides whether the thing carrying it
/// may happen.
///
/// Borrowed rather than owned, so `gate()` is free on every consumer and no
/// consumer has to store a second copy of its own fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gate<'a> {
    /// Flags that must all be set (DSL v0.3/v0.4).
    pub requires_flags: &'a [FlagId],
    /// Flags whose being set suppresses this (DSL v0.6).
    pub forbids_flags: &'a [FlagId],
    /// Numeric comparisons that must all hold (DSL v0.10, spec-0031).
    pub requires_state: &'a [StateCompare],
}

impl<'a> Gate<'a> {
    /// Build a gate from its three fields. The one constructor, so a consumer
    /// that forgets a field is a rustc error rather than a silently narrower
    /// gate.
    pub fn of(
        requires_flags: &'a [FlagId],
        forbids_flags: &'a [FlagId],
        requires_state: &'a [StateCompare],
    ) -> Self {
        Gate {
            requires_flags,
            forbids_flags,
            requires_state,
        }
    }

    /// The always-open gate: no flags, no comparison.
    pub const OPEN: Gate<'static> = Gate {
        requires_flags: &[],
        forbids_flags: &[],
        requires_state: &[],
    };

    /// True if this gate constrains nothing — the thing carrying it is
    /// unconditional, and emission writes it verbatim with no `execute` wrapper.
    pub fn is_empty(&self) -> bool {
        self.requires_flags.is_empty()
            && self.forbids_flags.is_empty()
            && self.requires_state.is_empty()
    }

    /// How many terms this gate has, across all three axes. The number a binding
    /// ledger reports.
    pub fn terms(&self) -> usize {
        self.requires_flags.len() + self.forbids_flags.len() + self.requires_state.len()
    }
}

// ---------------------------------------------------------------------------
// The six consumers, in one place
// ---------------------------------------------------------------------------
//
// Every object class that carries a gate answers `gate()`, and all six answers
// are written HERE rather than beside their own type. That is deliberate: the
// list of gate consumers is one fact, and a fact spread over six files is a fact
// nobody can read. A seventh consumer whose author forgets to add its `gate()`
// here is a consumer no proof written against `Gate` can see — which is exactly
// the shape `crates/dsl/tests/gate_consumers.rs` fails on.

impl crate::stages::Objective {
    /// This objective's whole gate, as one value (DSL v0.10).
    pub fn gate(&self) -> Gate<'_> {
        Gate::of(
            self.requires_flags(),
            self.forbids_flags(),
            self.requires_state(),
        )
    }
}

impl crate::stages::QuestEffect {
    /// This effect's whole gate, as one value (DSL v0.10).
    pub fn gate(&self) -> Gate<'_> {
        Gate::of(
            self.requires_flags(),
            self.forbids_flags(),
            self.requires_state(),
        )
    }
}

impl crate::stages::EnvTrigger {
    /// This trigger's whole gate, as one value (DSL v0.10).
    pub fn gate(&self) -> Gate<'_> {
        Gate::of(
            &self.requires_flags,
            &self.forbids_flags,
            &self.requires_state,
        )
    }
}

impl crate::stages::Trap {
    /// This trap's whole gate, as one value (DSL v0.10).
    pub fn gate(&self) -> Gate<'_> {
        Gate::of(
            &self.requires_flags,
            &self.forbids_flags,
            &self.requires_state,
        )
    }
}

impl crate::stages::DialogueOption {
    /// This option's whole gate, as one value (DSL v0.10).
    pub fn gate(&self) -> Gate<'_> {
        Gate::of(
            &self.requires_flags,
            &self.forbids_flags,
            &self.requires_state,
        )
    }
}

impl crate::stages::CastPlacement {
    /// This placement's whole gate, as one value (DSL v0.10).
    pub fn gate(&self) -> Gate<'_> {
        Gate::of(
            &self.requires_flags,
            &self.forbids_flags,
            &self.requires_state,
        )
    }
}

impl crate::stages::ShopOffer {
    /// This offer's whole gate, as one value (DSL v0.10, spec-0032) — a **price
    /// is a gate**, so a shop declares no comparison surface of its own.
    ///
    /// Defined beside the other six rather than on the type, for the reason the
    /// section header gives: the list of gate consumers is one fact.
    pub fn gate_view(&self) -> Gate<'_> {
        Gate::of(
            &self.requires_flags,
            &self.forbids_flags,
            &self.requires_state,
        )
    }
}

/// The object classes that carry a gate. **A closed set.**
///
/// `ALL` is the enumeration; [`GateConsumer::label`] and every consumer that
/// matches on one is exhaustive, so an eighth class is a compile error at every
/// site where the answer would have to change.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum GateConsumer {
    /// A stage-5 `quests[].objectives[]` — the gate decides whether the
    /// objective activates.
    Objective,
    /// Any `QuestEffect`, at any of the five effect roots, top-level or nested —
    /// the gate decides whether the effect's commands run.
    Effect,
    /// A stage-5 `triggers[]` — the gate decides whether the trigger can fire.
    Trigger,
    /// A stage-5 `traps[]` — the gate decides whether the trap is armed.
    Trap,
    /// A stage-6 `dialogues[].nodes[].options[]` — the gate decides whether the
    /// option is shown, and whether a direct `/trigger` on it does anything.
    DialogueOption,
    /// A stage-5 `quests[].cast[]` placement — the gate decides which branch's
    /// scene describes the world.
    CastPlacement,
    /// A stage-5 `shops[].offers[]` (DSL v0.10, spec-0032) — the gate decides
    /// whether the button is shown, and whether a direct `/trigger` on it does
    /// anything. **This is where a price lives**: a shop declares no comparison
    /// surface of its own, because "may this happen yet?" already has an owner.
    ShopOffer,
}

impl GateConsumer {
    /// Every consumer class, in enumeration order (= visit order in
    /// [`for_each_gate`]).
    pub const ALL: [GateConsumer; 7] = [
        GateConsumer::Objective,
        GateConsumer::Effect,
        GateConsumer::Trigger,
        GateConsumer::Trap,
        GateConsumer::DialogueOption,
        GateConsumer::CastPlacement,
        GateConsumer::ShopOffer,
    ];

    /// How many consumer classes there are.
    pub const COUNT: usize = Self::ALL.len();

    /// A short, stable label for a binding ledger or a diagnostic.
    pub fn label(self) -> &'static str {
        match self {
            GateConsumer::Objective => "objective",
            GateConsumer::Effect => "effect",
            GateConsumer::Trigger => "trigger",
            GateConsumer::Trap => "trap",
            GateConsumer::DialogueOption => "dialogue option",
            GateConsumer::CastPlacement => "cast placement",
            GateConsumer::ShopOffer => "shop offer",
        }
    }

    /// Whether emission evaluates this consumer's gate **against an acting
    /// player** (`@s`) rather than against the party holder (`#party`) — or
    /// `None` when the class alone cannot say.
    ///
    /// This is a statement about the emitter, not a preference: a dialogue
    /// option's availability is computed per player into `dw.dmask` and its
    /// `/trigger` handler runs `as @s`; a cast placement selects a scene into a
    /// per-player `dw.cast`. Three of the others are party predicates by
    /// construction — an objective's activation guard is read on the tick
    /// ("whoever finishes the last objective completes the quest for everyone"),
    /// and a trigger's and a trap's *arming* gates flip one global sentinel.
    ///
    /// **`Effect` answers `None`, and that is the whole point of the return
    /// type.** An effect's gate is evaluated wherever its bundle is run, and
    /// which that is belongs to the **root**, not to the effect: four roots have
    /// an acting player and three do not
    /// ([`EffectRootKind::runs_with_acting_player`](crate::EffectRootKind::runs_with_acting_player)),
    /// and on top of that the `sequence` / `on_arrive` seams inside a bundle drop
    /// the actor mid-walk. An earlier version of this method answered `true` for
    /// `Effect`, which is right for `on_objective_complete` and wrong for a
    /// trigger's effects, a trap's payload and a shortcut's `on_unlock` — three
    /// of the seven roots, silently. `Option` makes that wrong answer
    /// unrepresentable: a caller must handle the deferral.
    ///
    /// It is what makes a `player`-scoped datum's readability decidable
    /// (`DW0503`) from the closed consumer set rather than from a list somebody
    /// maintains — an eighth consumer class must answer this to compile.
    pub fn evaluates_per_player(self) -> Option<bool> {
        match self {
            // A shop offer's gate is computed into `dw.dmask` per player and its
            // `/trigger` handler runs `as @s`, exactly as a dialogue option's does
            // — which is what makes a `player`-scoped purse a legal price.
            GateConsumer::DialogueOption
            | GateConsumer::CastPlacement
            | GateConsumer::ShopOffer => Some(true),
            GateConsumer::Objective | GateConsumer::Trigger | GateConsumer::Trap => Some(false),
            // Ask the root (and then the seams inside the bundle).
            GateConsumer::Effect => None,
        }
    }

    /// The stage document this consumer lives in.
    pub fn stage(self) -> &'static str {
        match self {
            GateConsumer::Objective
            | GateConsumer::Trigger
            | GateConsumer::Trap
            | GateConsumer::CastPlacement
            | GateConsumer::ShopOffer => "quests",
            // An effect root hangs off the quests stage four times out of five and
            // off dialogue once; the site's own path says which.
            GateConsumer::Effect => "quests",
            GateConsumer::DialogueOption => "dialogue",
        }
    }
}

/// One gate the walk found: which consumer class it belongs to, and where.
pub struct GateSite {
    /// The object class carrying the gate.
    pub consumer: GateConsumer,
    /// JSON pointer to the carrying object within its stage document.
    pub path: String,
}

/// What a walk over the campaign's gates actually examined.
///
/// CLAUDE.md: *a green gate that binds to nothing is vacuous, not a pass.* A
/// proof over "every gate" is only as good as the consumer classes it reached
/// and the gates it found there; neither number is visible from the proof's own
/// output, so it is reported here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateBinding {
    /// How many of [`GateConsumer::COUNT`] classes the walk enumerated. Always
    /// `COUNT` for a walk that ran; asserted by [`for_each_gate`].
    pub consumers_enumerated: usize,
    /// Per class: how many gate-carrying objects this campaign actually has.
    pub sites: [(GateConsumer, usize); GateConsumer::COUNT],
    /// How many of those objects carry a **non-empty** gate.
    pub gated: usize,
    /// Total gate terms across every axis and every site.
    pub terms: usize,
}

impl GateBinding {
    /// A one-line, deterministic rendering for a report or a `--json` field.
    pub fn summary(&self) -> String {
        let per: Vec<String> = self
            .sites
            .iter()
            .map(|(k, n)| format!("{}={n}", k.label()))
            .collect();
        format!(
            "consumers {}/{}, sites {}, gated {}, terms {} [{}]",
            self.consumers_enumerated,
            GateConsumer::COUNT,
            self.sites.iter().map(|(_, n)| n).sum::<usize>(),
            self.gated,
            self.terms,
            per.join(", ")
        )
    }
}

/// Visit **every gate in the campaign**, in one fixed deterministic order, as
/// `f(&site, gate)`.
///
/// Order: every objective (quest order, objective order); every effect (via
/// [`crate::stages::for_each_campaign_effect`], which inherits the single effect-root
/// enumeration and descends nesting); every trigger; every trap; every dialogue
/// option; every cast placement.
///
/// Returns the [`GateBinding`] ledger.
///
/// # Panics
///
/// If the walk failed to enumerate all [`GateConsumer::COUNT`] classes.
/// Unreachable by construction and asserted anyway, for the same reason
/// [`crate::effects::for_each_effect_root`] asserts its own: a walk that quietly
/// stops visiting a class just answers a narrower question and stays green over
/// every campaign that does not use it.
pub fn for_each_gate(c: &Campaign, f: &mut dyn FnMut(&GateSite, Gate<'_>)) -> GateBinding {
    let mut sites = [
        (GateConsumer::Objective, 0usize),
        (GateConsumer::Effect, 0usize),
        (GateConsumer::Trigger, 0usize),
        (GateConsumer::Trap, 0usize),
        (GateConsumer::DialogueOption, 0usize),
        (GateConsumer::CastPlacement, 0usize),
        (GateConsumer::ShopOffer, 0usize),
    ];
    debug_assert_eq!(
        sites.map(|(k, _)| k),
        GateConsumer::ALL,
        "the binding ledger's slots are GateConsumer::ALL, in order"
    );
    let mut enumerated = [false; GateConsumer::COUNT];
    let mut gated = 0usize;
    let mut terms = 0usize;
    fn slot_of(k: GateConsumer) -> usize {
        GateConsumer::ALL
            .iter()
            .position(|x| *x == k)
            .expect("every consumer is a member of GateConsumer::ALL")
    }

    let mut visit = |consumer: GateConsumer,
                     path: String,
                     gate: Gate<'_>,
                     sites: &mut [(GateConsumer, usize); GateConsumer::COUNT],
                     gated: &mut usize,
                     terms: &mut usize| {
        sites[slot_of(consumer)].1 += 1;
        if !gate.is_empty() {
            *gated += 1;
        }
        *terms += gate.terms();
        f(&GateSite { consumer, path }, gate);
    };

    // C1 objectives.
    enumerated[slot_of(GateConsumer::Objective)] = true;
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            visit(
                GateConsumer::Objective,
                format!("/content/quests/{qi}/objectives/{oi}"),
                o.gate(),
                &mut sites,
                &mut gated,
                &mut terms,
            );
        }
    }
    // C2 effects — every root, top-level and nested, from the single enumeration.
    enumerated[slot_of(GateConsumer::Effect)] = true;
    crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
        visit(
            GateConsumer::Effect,
            path.to_string(),
            eff.gate(),
            &mut sites,
            &mut gated,
            &mut terms,
        );
    });
    // C3 triggers.
    enumerated[slot_of(GateConsumer::Trigger)] = true;
    for (ti, t) in c.quests.content.triggers.iter().enumerate() {
        visit(
            GateConsumer::Trigger,
            format!("/content/triggers/{ti}"),
            t.gate(),
            &mut sites,
            &mut gated,
            &mut terms,
        );
    }
    // C4 traps.
    enumerated[slot_of(GateConsumer::Trap)] = true;
    for (pi, p) in c.quests.content.traps.iter().enumerate() {
        visit(
            GateConsumer::Trap,
            format!("/content/traps/{pi}"),
            p.gate(),
            &mut sites,
            &mut gated,
            &mut terms,
        );
    }
    // C5 dialogue options.
    enumerated[slot_of(GateConsumer::DialogueOption)] = true;
    for (di, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (ni, node) in tree.nodes.iter().enumerate() {
            for (oi, opt) in node.options.iter().enumerate() {
                visit(
                    GateConsumer::DialogueOption,
                    format!("/content/dialogues/{di}/nodes/{ni}/options/{oi}"),
                    opt.gate(),
                    &mut sites,
                    &mut gated,
                    &mut terms,
                );
            }
        }
    }
    // C6 cast placements.
    enumerated[slot_of(GateConsumer::CastPlacement)] = true;
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (npc, entry) in &q.cast {
            for (pi, p) in entry.placements().iter().enumerate() {
                visit(
                    GateConsumer::CastPlacement,
                    format!("/content/quests/{qi}/cast/{}/{pi}", npc.as_str()),
                    p.gate(),
                    &mut sites,
                    &mut gated,
                    &mut terms,
                );
            }
        }
    }

    // C7 shop offers (DSL v0.10, spec-0032). A price is a gate term, so every
    // offer is visited here and nowhere else.
    enumerated[slot_of(GateConsumer::ShopOffer)] = true;
    for (si, shop) in c.quests.content.shops.iter().enumerate() {
        for (oi, off) in shop.offers.iter().enumerate() {
            visit(
                GateConsumer::ShopOffer,
                format!("/content/shops/{si}/offers/{oi}"),
                off.gate_view(),
                &mut sites,
                &mut gated,
                &mut terms,
            );
        }
    }

    let missed: Vec<&str> = GateConsumer::ALL
        .iter()
        .zip(enumerated)
        .filter(|(_, seen)| !*seen)
        .map(|(k, _)| k.label())
        .collect();
    assert!(
        missed.is_empty(),
        "for_each_gate enumerated {} of {} gate consumers — missing: {}. A consumer that stops \
         being enumerated has no other symptom.",
        GateConsumer::COUNT - missed.len(),
        GateConsumer::COUNT,
        missed.join(", ")
    );

    GateBinding {
        consumers_enumerated: GateConsumer::COUNT,
        sites,
        gated,
        terms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_gate_is_empty() {
        assert!(Gate::OPEN.is_empty());
        assert_eq!(Gate::OPEN.terms(), 0);
    }

    /// Every consumer names a stage and a label, and the stages are exactly the
    /// two stage documents gates live in.
    #[test]
    fn every_consumer_names_its_stage() {
        for k in GateConsumer::ALL {
            assert!(matches!(k.stage(), "quests" | "dialogue"), "{k:?}");
            assert!(!k.label().is_empty(), "{k:?}");
        }
    }
}
