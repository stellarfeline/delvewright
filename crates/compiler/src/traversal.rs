//! Body-traversal proof: a puppet's computed route may only contain moves the
//! **body walking it** could actually make (`DW0452` error, `DW0453` advisory).
//!
//! ## The defect this exists for (owner playtest, island round 21)
//!
//! Two sightings in one cutscene pair, both of the flock, both of `move-actor`
//! legs, and — this is the point — **neither one a body inside a block**.
//! [`crate::clearance`] proves where a body IS; this module proves what a body
//! DID to get there.
//!
//! * **Through.** At the ending, eight sheep walk through the mountain pen's
//!   fence gate. `[18, 73, -63]` holds
//!   `minecraft:oak_fence_gate[facing=east,open=false]` — a **closed** gate in a
//!   fence line that runs east-west. The owner could not walk straight through
//!   that cell herself; she had to offset to squeeze past the leaf. Sixteen
//!   walked legs cross it anyway. `DW0451` fired sixteen times and called it an
//!   advisory, on the reasoning that a barrier cell "is a narrow post or panel,
//!   so whether the body interpenetrates depends on sub-block shape the compiler
//!   does not carry". **That reasoning is about a body at rest.** For a body in
//!   motion the question is not whether it overlaps while standing, it is
//!   whether the route depends on something the body never does — and passing a
//!   closed fence gate depends on a right-click.
//!
//! * **Over.** At the outset, a sheep leaves the beach fold by climbing its
//!   wall: it hugs the pen's east face at `[7, 63, -9]`, rises a block straight
//!   up (a step-up's [`crate::nav::resample`] L is a vertical translation in
//!   place, by design — it is what keeps a body out of the step block's corner),
//!   crosses the north wall's top at `[7, 64, -10]` and drops into the meadow at
//!   `[7, 63, -11]`. **Nothing warned, and nothing would have**: the fold's ring
//!   is `minecraft:cobblestone_wall` at the corners and `minecraft:mossy_cobblestone`
//!   — a full cube, one block high — along the middle of each side, so the model
//!   sees a wall the body cannot pass at four cells and an ordinary one-block
//!   ledge it may hop at the rest. The pen's real opening, `[6, 63, -6]`, is
//!   never used. From the camera it reads as an animal walking up a stone wall.
//!
//! ## What the compiler already proved, and the gap
//!
//! [`crate::nav::World::standable_fp`] requires **solid** ground below, and
//! [`crate::nav::World::is_occupied`] counts a 1.5-tall barrier as impassable, so
//! a route can neither stand on a fence top nor walk through a fence. Those
//! rules hold. The two gaps are narrower and both are about **whose** rules they
//! are:
//!
//! 1. `is_occupied` deliberately excludes `use_gates` — a closed fence gate is
//!    passable **for the player**, who opens it with an adventure-legal
//!    right-click ([`crate::nav::World::without_gate_use`] exists precisely
//!    because an autonomous mob cannot). Scripted walks were routed on the
//!    player's rules; a scripted walk is a `tp` polyline and its body opens
//!    nothing.
//! 2. A barrier line whose courses are not all barriers is only a barrier where
//!    the barrier blocks are. Nothing related the two.
//!
//! ## The traversal model
//!
//! The traversal a route needs is compared against what the body walking it can
//! do — [`Traversal`], derived from the entity id, or **declared by the author**
//! and then proven (below).
//!
//! ## The author's side: a declaration the build holds you to (spec-0034)
//!
//! Spiders really do climb, so the rules here cannot be absolute — which means
//! there has to be a way to say "this body is an exception", or the exception
//! happens by accident and merely renders. That is
//! [`delvewright_dsl::BodyTraversal`], carried by every object class with a body,
//! a position and a compiler-emitted route: the stage-2 NPC and the stage-5
//! actor, through **one** shared type
//! ([`delvewright_dsl::body_traversal_sites`]) rather than a field per consumer.
//!
//! Three properties keep it a declaration rather than an opt-out, and each is
//! pinned by a test:
//!
//! 1. **It cannot reach the error tier.** [`Traversal::of_body`] takes
//!    `opens_gates` from the derivation and never from the author, and
//!    [`exempt_from_gate_rule`] is false for every class — so `DW0452` binds a
//!    declared body exactly as it binds an undeclared one. Wings, claws and
//!    paperwork all fail to open a fence gate.
//! 2. **It must change a verdict.** The surmount scan runs for every body,
//!    exempt or not, and the declaration is *exercised* only where the findings
//!    under the declared class differ from the findings under the derived one.
//!    A declaration that changes nothing is [`DW_TRAVERSAL_DECLARATION_INERT`],
//!    an error — so declaring `climber` on a sheep is accepted only where that
//!    sheep's route really does go over a barrier line. The comparison is
//!    written as a difference of verdicts, not as "is it a climber", so a second
//!    locomotion-governed rule joins it by existing.
//! 3. **A value the model cannot hold a body to is refused at declaration
//!    time**, not accepted and quietly ignored: `aquatic` governs no rule (it is
//!    a ledger label off vanilla's own tag), so declaring it is `DW0455` in the
//!    DSL crate, with the gap named — routing has one reachability model,
//!    standable ground, and flooded cells are impassable for every body.
//!
//! What a declaration deliberately does **not** do is change how a route is
//! computed. Every body is routed on ground rules, which was already true of a
//! derived class (a ghast actor walks), so the declaration inherits a documented
//! property rather than introducing folklore. It changes which rules examine the
//! body, and nothing else.
//!
//! And the proof cannot be dodged by a campaign that assembles nothing: this
//! module runs inside `emit`'s `assembles_world` arm, which is true whenever the
//! campaign has any NPC or actor ([`crate::clearance::has_bodies`]) — exactly the
//! condition under which a declaration can exist at all.
//!
//! ## Two rules, two different questions — and therefore two exemption axes
//!
//! This is the structural point, and getting it wrong is what an earlier draft
//! did (owner correction, island round 21). The two rules are not two strengths
//! of one rule; they ask different questions, so different things may excuse a
//! body from them.
//!
//! * **`DW0452` is a COLLISION-AND-INTERACTION question.** A closed fence gate's
//!   leaf spans the full cell across one axis, the planned route runs down the
//!   cell's centre line, and the body performs no right-click. **None of those
//!   three facts changes because a body has wings, climbs, or swims.** A
//!   `tp`-driven puppet is moved along the planned cell route whatever its body
//!   is; if that route enters a closed gate cell, the body passes through
//!   geometry it cannot pass through. So the rule binds to **every body**, and
//!   the only thing that could excuse it is [`Traversal::opens_gates`] — which is
//!   exactly why that is a per-body field and not a constant. **[`Locomotion`]
//!   does not touch this rule at all** ([`exempt_from_gate_rule`]).
//! * **`DW0453` is a LOCOMOTION question.** "Did this body go *over* a line it
//!   may not go *through*?" is only a defect for a body whose way past a wall is
//!   round it. Going over is precisely what a climber does, and a flier is not
//!   making a ground step-up in the first place. So this rule — and only this
//!   rule — is the one locomotion governs ([`exempt_from_surmount_rule`]).
//!
//! An earlier draft expressed the exemption as one early `continue` over the
//! whole body, before both rules. That conflated the two questions and let a
//! flying — or misclassified — body walk through a closed gate in silence. The
//! exemption is therefore expressed **per rule**, never per body, and the
//! ledger's `gate_use.cells` counts route cells for every non-gate-opening body
//! regardless of class, so the binding count itself shows the rule is total.
//!
//! ## The membership rule for this table, and the asymmetry behind it
//!
//! **The two errors are not symmetric.** Classifying a body too strictly costs a
//! false positive the owner dismisses in a minute of her QA hour. Classifying it
//! too loosely costs a body that is **never examined and reports green** — the
//! silent failure this whole module exists to end. So the table is built to fail
//! in the first direction:
//!
//! 1. **[`Locomotion::Ground`] is the default and the CHECKED class.** Every id
//!    vanilla data does not positively answer lands there, including ids the
//!    table has never heard of. Ambiguity — a mob that moves both ways, or only
//!    leaves the ground in some state — resolves to Ground.
//! 2. **A class may only carry an exemption when its membership is either
//!    vanilla's own answer or a closed, cited list whose exemption is
//!    advisory-tier.** Nothing gets a blanket exemption from the error tier.
//! 3. **Membership is decided by how the body MOVES, never by its name.**
//!
//! The classes, each with the test that decides it:
//!
//! * **Ground** — the default. Walks, steps and jumps. Includes, deliberately:
//!   `minecraft:breeze`, which reads like a flier and is not — it "moves around
//!   by hovering on the surface and by leaping", cannot rise into the air, and
//!   is fall-damage-immune purely because it lands hard
//!   ([Minecraft Wiki, *Breeze*](https://minecraft.wiki/w/Breeze)). It walks, so
//!   it is checked.
//! * **Climber** — vanilla's `Spider` class and its subclasses: `spider` and
//!   `cave_spider`, the only mobs whose `onClimbable()` is true whenever they
//!   are horizontally collided, which is the mechanic that lets them go up a
//!   sheer face. Exempt from **`DW0453` only** — going over is what a climber
//!   does — and never from `DW0452`, so a wrong entry here costs a missed
//!   advisory rather than a missed error. No vanilla tag answers this, so the
//!   list is closed, short and cited, per rule 2.
//! * **Flier** — leaves the ground under its own power and is not bound to a
//!   ground route: `allay`, `bat`, `bee`, `blaze`, `ender_dragon`, `ghast`,
//!   `happy_ghast`, `parrot`, `phantom`, `vex`, `wither`. Exempt from **`DW0453`
//!   only**, for the same reason and at the same tier as a climber. Notably
//!   **not** members: `breeze` (hops — see Ground), `chicken` and `parrot`'s
//!   slow-fall cousins in `#minecraft:fall_damage_immune`, which is a
//!   landing-damage tag and not a flight signal (it holds `breeze`, `chicken`,
//!   `cat`, `ocelot`, `iron_golem`, `magma_cube`, `shulker`); `shulker`, which
//!   is stationary and teleports. No vanilla tag answers "does this fly", so
//!   like `Climber` the list is closed, short and cited — permitted only because
//!   its exemption is advisory-tier (rule 2). A `Flier` is still fully bound by
//!   `DW0452`: wings do not open gates.
//! * **Aquatic** — membership is vanilla's own `#minecraft:aquatic` tag, read
//!   from the vendored registry ([`crate::registry::entity_in_tag`]), never a
//!   list here. Carries **no exemption at all**: it is a ledger classification,
//!   so a membership question can never cost a proof. (The hand list this
//!   replaced had eleven entries and vanilla's tag has fourteen — it was missing
//!   `turtle`, `nautilus` and `zombie_nautilus`. That is what a hand table does.)
//!
//! Every hand-listed id is additionally required to exist in the vendored
//! entity registry, so a typo or a species renamed at the next MC pin is a red
//! rather than a class that silently stopped matching.
//!
//! **Gate use** is `false` for every entity. No vanilla mob opens a fence gate —
//! villagers open *doors* — and a compiler-driven puppet performs no interaction
//! whatever. It is a field rather than a constant so the rule reads as the
//! capability claim it is, and so the player's own routing
//! ([`crate::nav::check_critical_path`]) stays visibly the one caller that has it.
//!
//! **What is deliberately NOT modelled**: per-entity jump reach. The router
//! measures every rise against the *player's* apex
//! ([`crate::nav::MAX_JUMP_RISE_16`]'s 1.25 blocks); a sheep matches it, but a
//! turtle does not, and the real `JUMP_STRENGTH` attribute defaults live in
//! server code rather than in any registry the compiler reads. A table of them
//! would be invented precision, so this proof does not assert on rise height —
//! it says so here, and [`TraversalGate`] states the axis as unbound so a reader
//! never has to infer it from silence.
//!
//! ## Two tiers, and why the line is where it is
//!
//! * **`DW0452` — error, a traversal the body cannot perform.** Today one bound
//!   rule: the route enters a **closed fence-gate** cell and the body does not
//!   open gates. There is no tolerance question — the cell's collision box spans
//!   the full cell across one axis and the route runs down the cell's centre
//!   line — and no shipped delve can open it later, because no runtime verb
//!   changes a fence gate's state. Build tier, like every other
//!   assembled-geometry defect. **No locomotion class is exempt from this rule**:
//!   a climber climbs and cannot open a gate either.
//! * **`DW0453` — warning, a barrier line surmounted over a full-cube course.**
//!   The route steps up onto a cell whose support is a full cube standing level
//!   with, and orthogonally beside, a 1.5-tall fence/wall cell, and comes back
//!   down within [`SURMOUNT_WINDOW`] steps — i.e. the body went **over** a line
//!   the same line refuses to let it walk **through**. The move itself is legal:
//!   a one-block rise is inside the player-class jump every body here has, and
//!   the compiler cannot know whether that course is a decorative kerb, a
//!   deliberate stile, or — as on the island — an enclosure the author drew as a
//!   wall and the router treated as a step. So it is a measurement with its
//!   cells named, judged in the owner's QA hour. A **climber** is exempt: going
//!   over is what a climber does.
//!
//! ## Prescription
//!
//! For `DW0452`: open the gate in the world the delve ships (a stage-7 edit
//! writing `open=true`), or wall the threshold and let the route find the way a
//! body can actually take. Never "just let it through" — the puppet's route is
//! the shot the party watches. For `DW0453`: make the line one material, so the
//! model's barrier and the player's eye agree, and let the route use the
//! opening.

use crate::failure::Failure;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use delvewright_dsl::Diagnostic;

use crate::nav::{ActorMovePlan, MovePlan, World};
use crate::plan::Plan;
use delvewright_dsl::{DwCode, ExitTier};

/// `DW0452`: a walked leg's route contains a move the body cannot make — today,
/// passing through a closed fence gate with no capability to open one.
pub const DW_TRAVERSAL_IMPOSSIBLE: DwCode = DwCode::new("DW0452", ExitTier::Build);

/// `DW0453`: a walked leg's route goes **over** a barrier line by stepping onto a
/// full-cube course of it. Advisory: the step is physically legal, and whether
/// the course is a kerb or an enclosure is a content judgement.
pub const DW_BARRIER_SURMOUNTED: DwCode = DwCode::new("DW0453", ExitTier::Build);

/// `DW0454`: a body's `traversal` declaration is **inert** — it changed no
/// rule's verdict, so nothing in this build holds the body to it (spec-0034).
///
/// [`Binds::EveryVersion`](delvewright_dsl::Binds::EveryVersion), like its two
/// neighbours: the verdict is a function of the campaign alone — the author's
/// own declaration set against the world the author built — so there is nothing
/// to grandfather. The surface that carries the declaration is fenced per stage
/// at 0.11 by `DW0141`, so a campaign below 0.11 cannot raise this code at all.
pub const DW_TRAVERSAL_DECLARATION_INERT: DwCode = DwCode::new("DW0454", ExitTier::Build);

/// How many route steps after a rise still count as "and came down the other
/// side". Four: the island's crossing takes one step up, at most two along the
/// top and one down, and a window this short is what keeps an ordinary ledge the
/// route genuinely uses (climb up, walk, stay up) out of the tier.
pub const SURMOUNT_WINDOW: usize = 4;

/// How a body gets around. **The vocabulary lives in the DSL crate** since
/// spec-0034 made it authorable ([`delvewright_dsl::BodyTraversal`]) — one enum,
/// so what an author declares and what this module derives can never mean two
/// different things. The derivation table, the membership rule and the asymmetry
/// it is built around stay here; see the module docs.
pub use delvewright_dsl::{BodyRef, BodyTraversal, Locomotion};

/// Which locomotion classes [`DW_TRAVERSAL_IMPOSSIBLE`] declines to examine.
///
/// **None — locomotion does not touch this rule** (owner correction, island
/// round 21). `DW0452` is a collision-and-interaction question, not a locomotion
/// one: the gate leaf spans the full cell across one axis, the planned route
/// runs down the cell's centre line, and the body performs no right-click, and
/// not one of those three facts changes because the body has wings or claws. A
/// `tp`-driven puppet is moved along the planned cell route whatever its body
/// is. The only thing that can excuse this rule is [`Traversal::opens_gates`],
/// which is why that is a per-body field rather than a constant.
///
/// Written as a function taking the class it ignores — rather than as an absent
/// `if` — so "no class is exempt here" is a claim a test can hold, and so a
/// future edit that wires locomotion back into the error tier has to delete an
/// assertion to do it.
fn exempt_from_gate_rule(_class: Locomotion) -> bool {
    false
}

/// Which locomotion classes [`DW_BARRIER_SURMOUNTED`] declines to examine:
/// climbers and fliers.
///
/// This is the rule locomotion legitimately governs. "Did this body go *over* a
/// line it may not go *through*?" is only a defect for a body whose way past a
/// wall is round it: going over is precisely what a climber does, and a flier is
/// not making a ground step-up in the first place. Advisory tier — which is the
/// only tier a hand-listed class is permitted to gate (module docs, rule 2), so
/// a wrong entry costs a missed advisory and never a missed error.
fn exempt_from_surmount_rule(class: Locomotion) -> bool {
    matches!(class, Locomotion::Climber | Locomotion::Flier)
}

/// Vanilla's own aquatic tag, vendored (`crate::registry::entity_tags`).
const AQUATIC_TAG: &str = "minecraft:aquatic";

/// The mobs that leave the ground under their own power and are not bound to a
/// ground route.
///
/// No vanilla `entity_type` tag answers "does this fly".
/// `#minecraft:fall_damage_immune` is the one that looks like it and is not — it
/// is a landing-damage tag, and it holds `breeze`, `chicken`, `cat`, `ocelot`,
/// `iron_golem`, `magma_cube` and `shulker`. So this is a closed, cited list,
/// permitted only because it gates the **advisory** tier
/// ([`exempt_from_surmount_rule`]) and never the error tier (module docs,
/// rule 2). Deliberately excluded: `breeze` (hops — <https://minecraft.wiki/w/Breeze>),
/// `chicken` (flaps to slow a fall, cannot ascend), `shulker` (stationary,
/// teleports). Ambiguity resolves to [`Locomotion::Ground`], the checked class.
const FLIERS: [&str; 11] = [
    "minecraft:allay",
    "minecraft:bat",
    "minecraft:bee",
    "minecraft:blaze",
    "minecraft:ender_dragon",
    "minecraft:ghast",
    "minecraft:happy_ghast",
    "minecraft:parrot",
    "minecraft:phantom",
    "minecraft:vex",
    "minecraft:wither",
];

/// The mobs that climb sheer surfaces: vanilla's `Spider` and its subclasses,
/// the only ones whose `onClimbable()` is true whenever they are horizontally
/// collided. No vanilla `entity_type` tag answers this — `#minecraft:arthropod`
/// is a damage-bonus tag holding `bee`, `silverfish` and `endermite` too — so
/// this is a closed, cited list rather than derived data, and it is allowed to
/// be one only because it grants an **advisory-tier** exemption and never an
/// error-tier one (module docs, rule 2).
/// <https://minecraft.wiki/w/Spider>
const CLIMBERS: [&str; 2] = ["minecraft:spider", "minecraft:cave_spider"];

/// What a body can do when it moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Traversal {
    /// How it gets around.
    pub locomotion: Locomotion,
    /// Right-clicks a fence gate open. **False for every entity**: no vanilla mob
    /// does it, and a `tp`-driven puppet performs no interaction at all. Only the
    /// player has it, and the player is not a body this proof quantifies over.
    pub opens_gates: bool,
}

impl Traversal {
    /// The traversal capabilities the build actually uses for a body: its
    /// declaration if it made one, else what its species implies.
    ///
    /// `opens_gates` is taken from the derivation in **both** cases and is never
    /// authorable — see [`Traversal::opens_gates`] and the module docs. A
    /// declaration cannot reach the error tier.
    pub fn of_body(entity: &str, declared: Option<Locomotion>) -> Traversal {
        let derived = Traversal::of_entity(entity);
        Traversal {
            locomotion: declared.unwrap_or(derived.locomotion),
            opens_gates: derived.opens_gates,
        }
    }

    /// The traversal capabilities of `entity`, by vanilla behaviour.
    ///
    /// Every id this cannot positively answer falls to [`Locomotion::Ground`] —
    /// the **checked** class. That is the load-bearing safety property of the
    /// whole module (see the module docs' asymmetry), so it is pinned by test
    /// rather than left to this comment.
    pub fn of_entity(entity: &str) -> Traversal {
        let id = crate::registry::namespaced_entity(entity);
        let locomotion = if CLIMBERS.contains(&id.as_str()) {
            Locomotion::Climber
        } else if FLIERS.contains(&id.as_str()) {
            Locomotion::Flier
        } else if crate::registry::entity_in_tag(&id, AQUATIC_TAG) {
            Locomotion::Aquatic
        } else {
            Locomotion::Ground
        };
        Traversal {
            locomotion,
            opens_gates: false,
        }
    }
}

/// The binding ledger (`docs/reference/playtest-methodology.md` rule 1): what
/// this proof actually looked at, said out loud, so a green can never be read as
/// a pass over bodies it never examined.
///
/// The capability axis is its own way to bind to nothing: a class that carries
/// an exemption is a class this proof does not examine, so a total alone would
/// report green over exactly the bodies it understands least. Hence a count per
/// [`Locomotion`] class, not just a total — and a per-rule count beneath it, so
/// "no findings" and "nothing tested" stay distinguishable rule by rule.
#[derive(Clone, Debug, Default)]
pub struct TraversalGate {
    /// Walked legs examined (`move-npc` + `move-actor`, every planned driver).
    pub legs: usize,
    /// Route cells examined across those legs.
    pub route_cells: usize,
    /// Legs per [`Locomotion`] class token.
    pub legs_by_class: BTreeMap<&'static str, usize>,
    /// Route cells tested against the gate-use rule (`DW0452`) — i.e. cells
    /// walked by a body that cannot open gates.
    pub gate_rule_cells: usize,
    /// Rises tested against the surmount rule (`DW0453`).
    pub surmount_rule_rises: usize,
    /// The author-declared side of the proof (spec-0034).
    pub declared: DeclaredLedger,
}

/// What the campaign's own `traversal` declarations bought — the second binding
/// axis spec-0034 introduces.
///
/// A declaration silences a rule for a body, so counting only what the rules
/// examined would report green over exactly the bodies an author asked the proof
/// to treat differently. `bodies` says how many made a claim, `by_class` under
/// which token, `exercised` how many the world actually paid for, and
/// `advisories_waived` how many `DW0453` findings the claims removed — so a
/// reader can see the cost of every exemption this build granted.
///
/// `exercised` equals `bodies` in any build that succeeds: an unexercised
/// declaration is `DW0454`. It is still reported, because the ledger is also
/// read off a build that is being debugged.
#[derive(Clone, Debug, Default)]
pub struct DeclaredLedger {
    /// Bodies carrying a `traversal` declaration.
    pub bodies: usize,
    /// Declared bodies per declared [`Locomotion`] token.
    pub by_class: BTreeMap<&'static str, usize>,
    /// Declarations that changed at least one rule's verdict for their body.
    pub exercised: usize,
    /// `DW0453` advisories a declaration removed.
    pub advisories_waived: usize,
}

impl TraversalGate {
    /// Whether this proof matched nothing at all.
    pub fn unbound(&self) -> bool {
        self.legs == 0
    }

    /// The ledger as the `validation/traversal-gate.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        let mut by_class = serde_json::Map::new();
        for class in Locomotion::ALL {
            by_class.insert(
                class.token().to_string(),
                serde_json::json!(self.legs_by_class.get(class.token()).copied().unwrap_or(0)),
            );
        }
        let mut declared_by_class = serde_json::Map::new();
        for class in Locomotion::ALL {
            declared_by_class.insert(
                class.token().to_string(),
                serde_json::json!(
                    self.declared
                        .by_class
                        .get(class.token())
                        .copied()
                        .unwrap_or(0)
                ),
            );
        }
        let mut v = serde_json::json!({
            "legs": self.legs,
            "route_cells": self.route_cells,
            "legs_by_class": by_class,
            // spec-0034: what the campaign's own declarations claimed and what
            // the world paid for them. A declaration SILENCES a rule, so a
            // ledger that counted only what the rules examined would report
            // green over exactly the bodies an author asked to be treated
            // differently.
            "declared": {
                "bodies": self.declared.bodies,
                "by_class": declared_by_class,
                "exercised": self.declared.exercised,
                "advisories_waived": self.declared.advisories_waived,
                "code": DW_TRAVERSAL_DECLARATION_INERT,
            },
            "rules": {
                "gate_use": { "code": DW_TRAVERSAL_IMPOSSIBLE, "cells": self.gate_rule_cells },
                "surmount": { "code": DW_BARRIER_SURMOUNTED, "rises": self.surmount_rule_rises },
                // Stated, not asserted: see the module docs. A reader must not
                // have to infer from silence that rise height went unchecked.
                "jump_reach": {
                    "bound": false,
                    "reason": "per-entity JUMP_STRENGTH is server-code attribute data, not \
                               registry data the compiler reads; every rise is measured against \
                               the PLAYER's apex (nav::MAX_JUMP_RISE_16) for every body",
                },
            },
            "unbound": self.unbound(),
        });
        if self.unbound() {
            v["reason"] = serde_json::json!(
                "the campaign plans no walked leg — no `move-npc` and no `move-actor` reached \
                 the router, so no body's traversal was examined"
            );
        }
        v
    }
}

/// One walked leg, flattened out of the two plan kinds.
struct Leg<'a> {
    /// `move-npc` or `move-actor`.
    verb: &'static str,
    /// The moving body's declared id.
    id: &'a str,
    /// The destination anchor id.
    to_anchor: &'a str,
    /// The A* cell route, start to target inclusive.
    cells: &'a [[i32; 3]],
    /// The entity whose body (and capabilities) the puppet wears.
    entity: String,
    /// The locomotion this body's author DECLARED, if any (spec-0034). `None` =
    /// the class derived from [`Leg::entity`].
    declared: Option<Locomotion>,
    /// JSON pointer at the declaration, for a warning's diagnostic path.
    path: String,
    /// The DSL stage the declaration lives in.
    stage: &'static str,
}

/// Every planned walked leg, in plan order, paired with the body that walks it.
/// A leg whose mover is not declared is skipped — `DW0325`/`DW0345` own dangling
/// references.
fn legs<'a>(
    plan: &'a Plan,
    moves: &'a [MovePlan],
    actor_moves: &'a [ActorMovePlan],
) -> Vec<Leg<'a>> {
    let mut out = Vec::new();
    for m in moves {
        let Some((i, n)) = plan
            .campaign
            .npcs
            .content
            .npcs
            .iter()
            .enumerate()
            .find(|(_, n)| n.id.as_str() == m.npc)
        else {
            continue;
        };
        out.push(Leg {
            verb: "move-npc",
            id: &m.npc,
            to_anchor: &m.to_anchor,
            cells: &m.cells,
            entity: crate::nav::npc_body_entity(n),
            declared: n.traversal.map(|t| t.locomotion),
            path: format!("/content/npcs/{i}"),
            stage: "npcs",
        });
    }
    for m in actor_moves {
        let Some((i, a)) = plan
            .campaign
            .quests
            .content
            .actors
            .iter()
            .enumerate()
            .find(|(_, a)| a.id.as_str() == m.actor)
        else {
            continue;
        };
        out.push(Leg {
            verb: "move-actor",
            id: &m.actor,
            to_anchor: &m.to_anchor,
            cells: &m.cells,
            entity: crate::nav::actor_body_entity(a),
            declared: a.traversal.map(|t| t.locomotion),
            path: format!("/content/actors/{i}"),
            stage: "quests",
        });
    }
    out
}

/// Whether `support` is a **full cube** standing level with, and orthogonally
/// beside, a 1.5-tall barrier cell — i.e. a course of a barrier line.
///
/// Full cube specifically: a slab or a `dirt_path` beside a fence is a floor
/// detail, not a course of the wall, and a body crossing one has crossed
/// nothing. Neighbours are the four cardinals at the support's own level, which
/// is what makes "the same line" mean the same line rather than "there is a
/// fence somewhere nearby".
fn barrier_course(world: &World, support: [i32; 3]) -> Option<[i32; 3]> {
    if world.solid_top_16(support) != Some(crate::assembled::FULL_HEIGHT_16) {
        return None;
    }
    [(-1, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .map(|(dx, dz)| [support[0] + dx, support[1], support[2] + dz])
        .find(|&n| world.is_barrier(n))
}

/// Prove every walked leg's route contains only moves its body can make
/// (`DW0452`, `DW0453`).
///
/// Returns the advisories and the binding ledger on success; ANY error-tier
/// violation fails the build, with every one named in a single message so one
/// build gives the whole fix list. A leg reports at most its FIRST offending
/// cell per rule: a herd driven through one gate is one defect, not sixteen
/// ticks of one.
pub fn check_traversal(
    plan: &Plan,
    world: &World,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
) -> Result<(Vec<Diagnostic>, TraversalGate), Failure> {
    let mut gate = TraversalGate::default();
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<Diagnostic> = Vec::new();
    // Every DECLARED body in the campaign, whether or not it ever walks — a
    // declaration on a body that never moves is exactly as inert as one whose
    // route never uses it, and enumerating from the legs would miss it entirely.
    let mut declared = declared_bodies(plan);
    for (key, d) in &declared {
        let _ = key;
        gate.declared.bodies += 1;
        *gate
            .declared
            .by_class
            .entry(d.declared.token())
            .or_insert(0) += 1;
    }
    for leg in legs(plan, moves, actor_moves) {
        let derived = Traversal::of_entity(&leg.entity).locomotion;
        let cap = Traversal::of_body(&leg.entity, leg.declared);
        gate.legs += 1;
        gate.route_cells += leg.cells.len();
        *gate
            .legs_by_class
            .entry(cap.locomotion.token())
            .or_insert(0) += 1;
        // --- DW0452: a traversal this body cannot perform. -------------------
        //
        // `cap.opens_gates` is derived and never authorable, and no locomotion
        // class is exempt, so a declaration provably cannot reach this tier.
        if !cap.opens_gates && !exempt_from_gate_rule(cap.locomotion) {
            gate.gate_rule_cells += leg.cells.len();
            if let Some(&cell) = leg.cells.iter().find(|&&c| world.is_use_gate(c)) {
                errors.push(gate_violation(&leg, cell));
            }
        }
        // --- DW0453: a barrier line surmounted over a full-cube course. ------
        //
        // The scan runs for EVERY body, exempt or not, because a declaration is
        // only honest if the build can say what it changed: the finding is what
        // the effective class then suppresses or raises, and the difference
        // between the two classes is what makes the declaration exercised.
        let scan = scan_surmounts(world, leg.cells);
        let derived_silent = exempt_from_surmount_rule(derived);
        let effective_silent = exempt_from_surmount_rule(cap.locomotion);
        if !effective_silent {
            gate.surmount_rule_rises += scan.rises;
            if let Some((from, support, barrier)) = scan.first {
                warnings.push(surmount_advisory(&leg, from, support, barrier));
            }
        }
        if let Some(d) = declared.get_mut(&decl_key(leg.stage, leg.id)) {
            d.legs += 1;
            // The declaration is EXERCISED when it changes a verdict — stated as
            // a comparison of the findings the two classes produce, not as "is
            // it a climber", so a second locomotion-governed rule joins this
            // test by existing rather than by being remembered here.
            if scan.first.is_some() && derived_silent != effective_silent {
                d.exercised = true;
                if effective_silent {
                    gate.declared.advisories_waived += 1;
                }
            }
        }
    }
    if let Some(first) = errors.first() {
        let mut message = first.clone();
        if errors.len() > 1 {
            let _ = write!(
                message,
                " {} further impossible-traversal violation(s) in this build:",
                errors.len() - 1
            );
            for e in &errors[1..] {
                let _ = write!(message, " {e};");
            }
        }
        return Err(Failure {
            code: DW_TRAVERSAL_IMPOSSIBLE,
            message,
        });
    }
    gate.declared.exercised = declared.values().filter(|d| d.exercised).count();
    // --- DW0454: a declaration nothing in this build holds the body to. ------
    let inert: Vec<String> = declared
        .values()
        .filter(|d| !d.exercised)
        .map(inert_declaration)
        .collect();
    if let Some(first) = inert.first() {
        let mut message = first.clone();
        if inert.len() > 1 {
            let _ = write!(
                message,
                " {} further inert traversal declaration(s) in this build:",
                inert.len() - 1
            );
            for e in &inert[1..] {
                let _ = write!(message, " {e};");
            }
        }
        return Err(Failure {
            code: DW_TRAVERSAL_DECLARATION_INERT,
            message,
        });
    }
    Ok((warnings, gate))
}

/// What the surmount scan found on one route: how many rises it looked at, and
/// the FIRST rise that crossed a barrier line over a full-cube course.
///
/// Split out of the rule so it can run for every body regardless of class. A
/// declaration that exempts a body has to be shown to have exempted something —
/// which means the finding must be computed even where it is then suppressed.
struct SurmountScan {
    /// Rises in the route (a step onto a higher cell).
    rises: usize,
    /// `(from, support, barrier)` of the first crossing, if any. A leg reports
    /// at most one: a herd over one wall is one defect, not sixteen ticks of it.
    first: Option<([i32; 3], [i32; 3], [i32; 3])>,
}

/// Scan a route for barrier-line crossings — the class-independent half of
/// `DW0453`.
fn scan_surmounts(world: &World, cells: &[[i32; 3]]) -> SurmountScan {
    let mut scan = SurmountScan {
        rises: 0,
        first: None,
    };
    for i in 0..cells.len().saturating_sub(1) {
        let (from, onto) = (cells[i], cells[i + 1]);
        if onto[1] <= from[1] {
            continue;
        }
        scan.rises += 1;
        if scan.first.is_some() {
            continue;
        }
        let support = [onto[0], onto[1] - 1, onto[2]];
        let Some(barrier) = barrier_course(world, support) else {
            continue;
        };
        // …and came down the other side. A body that climbs onto a ledge and
        // stays there has used the ledge, not crossed the line.
        let end = (i + 1 + SURMOUNT_WINDOW).min(cells.len());
        if !cells[i + 2..end].iter().any(|c| c[1] <= from[1]) {
            continue;
        }
        scan.first = Some((from, support, barrier));
    }
    scan
}

/// One body that declared a `traversal`, and what this build did with it.
struct DeclaredBody {
    /// `npcs` / `quests`.
    stage: &'static str,
    /// JSON pointer at the `traversal` field.
    path: String,
    /// The body's id.
    id: String,
    /// The entity whose body actually ships (mannequin rule applied).
    entity: String,
    /// The class the author declared.
    declared: Locomotion,
    /// The class the entity implies.
    derived: Locomotion,
    /// Walked legs this body has.
    legs: usize,
    /// Whether the declaration changed a verdict on any of them.
    exercised: bool,
}

/// The `(stage, id)` key a declaration and a leg are matched on.
fn decl_key(stage: &str, id: &str) -> String {
    format!("{stage}\u{1}{id}")
}

/// Every body in the campaign that declares a `traversal`, from the DSL's own
/// closed enumeration of the declaration's consumers
/// ([`delvewright_dsl::body_traversal_sites`]) rather than from a walk this
/// module remembers to keep in step.
fn declared_bodies(plan: &Plan) -> BTreeMap<String, DeclaredBody> {
    let mut out = BTreeMap::new();
    for site in delvewright_dsl::body_traversal_sites(plan.campaign) {
        // The mannequin rule is the compiler's (`nav::npc_body_entity`), so the
        // match lives here — and it is a match on a closed sum type, so a third
        // body class cannot be added without this line failing to compile.
        let entity = match site.body {
            BodyRef::Npc(n) => crate::nav::npc_body_entity(n),
            BodyRef::Actor(a) => crate::nav::actor_body_entity(a),
        };
        let derived = Traversal::of_entity(&entity).locomotion;
        out.insert(
            decl_key(site.body.stage(), site.body.id()),
            DeclaredBody {
                stage: site.body.stage(),
                path: site.path.clone(),
                id: site.body.id().to_string(),
                entity,
                declared: site.traversal.locomotion,
                derived,
                legs: 0,
                exercised: false,
            },
        );
    }
    out
}

/// The `DW0454` message: the declaration bought nothing, so nothing holds the
/// body to it.
///
/// Three distinguishable shapes, because the fix differs: the body never walks
/// at all; the body walks but no route makes the move the class governs; or the
/// declared class is the one the species already had.
fn inert_declaration(d: &DeclaredBody) -> String {
    let (declared, derived) = (d.declared.token(), d.derived.token());
    let why = if d.declared == d.derived {
        format!(
            "`{}` is already a `{derived}` by its entity id, so the declaration restates what the \
             compiler derives and changes nothing",
            d.entity
        )
    } else if d.legs == 0 {
        "this body walks no leg at all — no `move-npc` and no `move-actor` ever routes it — so \
         there is no move for a locomotion class to govern"
            .to_string()
    } else {
        format!(
            "every leg this body walks earns exactly the same verdicts under `{declared}` as under \
             the `{derived}` its entity id implies: no route of its goes OVER a barrier line, \
             which is the only move locomotion governs (`{DW_BARRIER_SURMOUNTED}`)"
        )
    };
    format!(
        "{} `{}` ({}) at `{}` declares `traversal.locomotion: {declared}`, and the declaration is \
         INERT: {why}. A declaration is a claim the build holds the body to, never a way to switch a \
         check off — that is the whole point of being able to declare one — so a claim nothing in \
         this build can pay for is refused rather than recorded. Note what this rule is NOT: \
         `{DW_TRAVERSAL_IMPOSSIBLE}`, the error tier, has no authorable exemption at all (passing \
         a closed fence gate is a right-click no puppet makes, whatever its body), so no \
         declaration was ever going to be answered there. Prescription: either remove the \
         declaration, or build the world that needs it — if this body really is meant to go over \
         a wall the party has to walk round, give it the route that does, and the declaration is \
         then what makes that route legal instead of a finding.",
        if d.stage == "npcs" { "npc" } else { "actor" },
        d.id,
        d.entity,
        d.path,
    )
}

/// How a walking body is described in a message.
fn describe(leg: &Leg) -> String {
    let kind = if leg.verb == "move-npc" {
        "npc"
    } else {
        "actor"
    };
    format!("{kind} `{}` ({})", leg.id, leg.entity)
}

/// The `DW0452` message: the route needs a right-click the body never makes.
fn gate_violation(leg: &Leg, cell: [i32; 3]) -> String {
    format!(
        "{} walks its `{}` leg to `{}` THROUGH THE CLOSED FENCE GATE at {cell:?}. Passing a closed \
         fence gate is a right-click USE. The player has it — which is why the occupancy model \
         treats a gate cell as a walkable edge for the critical path — but this body does not: no \
         vanilla mob opens a fence gate, and a scripted walk is a compiler-emitted `tp` polyline \
         whose puppet performs no interaction at all. Nor can the delve open it later: no runtime \
         verb changes a fence gate's block state, so a gate that ships `open=false` is closed for \
         the whole delve. The body therefore slides through a barrier the party cannot walk \
         through, in a staged shot the party is watching. \
         Prescription: ship the gate OPEN (a stage-7 `world-edits` fill writing `open=true` on that \
         cell — an open fence gate has no collision at all, so the same route becomes honest for \
         puppet and player alike), or seal the threshold and let the route find the way a body can \
         actually take. Do NOT leave it to the beat's fiction: nothing in the campaign proves a \
         gate open, and this is the cell where that assumption was visible in play.",
        describe(leg),
        leg.verb,
        leg.to_anchor,
    )
}

/// The `DW0453` advisory: the route went over a line it may not go through.
fn surmount_advisory(
    leg: &Leg,
    from: [i32; 3],
    support: [i32; 3],
    barrier: [i32; 3],
) -> Diagnostic {
    let body = describe(leg);
    let verb = leg.verb;
    let to = leg.to_anchor;
    let text = format!(
        "{body} walks its `{verb}` leg to `{to}` OVER a barrier line: from {from:?} it steps up onto the \
         full-cube block at {support:?} and comes back down on the far side, and {support:?} \
         stands level with and directly beside the 1.5-tall fence/wall cell at {barrier:?}. The \
         line is impassable where the barrier blocks are and one step high here, so the enclosure \
         the content draws is not the enclosure the router sees — the body goes over the wall \
         instead of round to the opening. Advisory, not an error: a one-block rise is inside the \
         jump every body in the dims table has, and the compiler cannot tell a decorative kerb or \
         a deliberate stile from an enclosure that was meant to hold. Judged in playtest. \
         Prescription: build the line out of ONE material so the model's barrier and the player's \
         eye agree, and let the route use the opening. If instead this body really is meant to go \
         over walls — a spider-sheep, a thing that climbs in your fiction whatever its entity id \
         says — DECLARE it: `\"traversal\": {{\"locomotion\": \"climber\"}}` on the body \
         (dsl_version 0.11.0). That is not a way to switch this line off: the build then requires \
         the route to really make the move, and refuses a declaration that buys nothing \
         ({DW_TRAVERSAL_DECLARATION_INERT})."
    );
    Diagnostic::warning(DW_BARRIER_SURMOUNTED, leg.stage, leg.path.clone(), text)
}

/// Whether the campaign plans any walked leg at all — the cheap gate that keeps a
/// walk-free campaign on its exact pre-existing build path.
pub fn has_legs(moves: &[MovePlan], actor_moves: &[ActorMovePlan]) -> bool {
    !moves.is_empty() || !actor_moves.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The two rules read cells, so the fixtures are cell sets — the same shape
    /// `clearance`'s unit tests use.
    fn world(
        solid: BTreeSet<[i32; 3]>,
        tall: BTreeSet<[i32; 3]>,
        gates: BTreeSet<[i32; 3]>,
    ) -> World {
        World::from_occupancy(
            crate::assembled::Occupancy {
                solid,
                tall,
                use_gates: gates,
                flooded: BTreeSet::new(),
                partial: BTreeMap::new(),
            },
            crate::nav::Premises::geometry_only(),
        )
    }

    /// A floor at `y-1` over an 8×8 patch.
    fn floor(y: i32) -> BTreeSet<[i32; 3]> {
        let mut s = BTreeSet::new();
        for x in 0..8 {
            for z in 0..8 {
                s.insert([x, y - 1, z]);
            }
        }
        s
    }

    #[test]
    fn codes_are_dw0452_and_dw0453() {
        assert_eq!(DW_TRAVERSAL_IMPOSSIBLE, "DW0452");
        assert_eq!(DW_BARRIER_SURMOUNTED, "DW0453");
    }

    /// Membership is decided by how a body MOVES, per class, from its stated
    /// source — a spider climbs, a squid is in vanilla's aquatic tag, a sheep
    /// walks — and **nothing** opens a fence gate.
    #[test]
    fn capabilities_come_from_the_entity() {
        assert_eq!(
            Traversal::of_entity("minecraft:spider").locomotion,
            Locomotion::Climber
        );
        assert_eq!(
            Traversal::of_entity("minecraft:dolphin").locomotion,
            Locomotion::Aquatic
        );
        assert_eq!(
            Traversal::of_entity("minecraft:sheep").locomotion,
            Locomotion::Ground
        );
        for e in [
            "minecraft:sheep",
            "minecraft:spider",
            "minecraft:villager",
            "minecraft:dolphin",
            "minecraft:ghast",
        ] {
            assert!(
                !Traversal::of_entity(e).opens_gates,
                "no vanilla body opens a fence gate: {e}"
            );
        }
    }

    /// **The module's load-bearing safety property**: anything this table cannot
    /// positively answer is `Ground`, which is the CHECKED class. A comment is
    /// not a contract — the repo has paid for that shape before — so the
    /// fallback is pinned in every form it can arrive in.
    #[test]
    fn every_unanswered_id_falls_to_the_checked_class() {
        for id in [
            "minecraft:mannequin",  // a real body the delves ship
            "minecraft:not_a_mob",  // never existed
            "delvewright:invented", // another namespace entirely
            "sheep",                // un-namespaced, must still resolve
            "",                     // degenerate
            "minecraft:chicken",    // flaps, cannot ascend -> the checked class
            "minecraft:shulker",    // stationary, teleports -> the checked class
        ] {
            assert_eq!(
                Traversal::of_entity(id).locomotion,
                Locomotion::Ground,
                "`{id}` must land in the checked class, not be exempted"
            );
        }
    }

    /// The breeze specifically, because it is the one that got this wrong: it
    /// reads like a flier and hops like a ground mob. Vanilla: it "moves around
    /// by hovering on the surface and by leaping" and cannot rise into the air —
    /// <https://minecraft.wiki/w/Breeze>. It is a ground body, so it owes the
    /// surmount advisory like any other.
    #[test]
    fn a_breeze_walks_and_is_therefore_checked() {
        assert_eq!(
            Traversal::of_entity("minecraft:breeze").locomotion,
            Locomotion::Ground
        );
        assert!(!exempt_from_surmount_rule(
            Traversal::of_entity("minecraft:breeze").locomotion
        ));
    }

    /// A flier is a flier where that means something and nowhere else.
    #[test]
    fn a_flier_skips_the_surmount_rule_and_nothing_else() {
        for id in ["minecraft:ghast", "minecraft:bat", "minecraft:phantom"] {
            let cap = Traversal::of_entity(id);
            assert_eq!(cap.locomotion, Locomotion::Flier, "{id}");
            assert!(exempt_from_surmount_rule(cap.locomotion), "{id}");
            assert!(
                !exempt_from_gate_rule(cap.locomotion) && !cap.opens_gates,
                "`{id}` has wings, not hands: the gate rule still binds it"
            );
        }
    }

    /// The **exemption matrix**, pinned per rule — the structural property the
    /// owner's round-21 correction is about, and the one a future edit is most
    /// likely to undo by accident.
    ///
    /// `DW0452` is a collision-and-interaction question, so **no** locomotion
    /// class is exempt: a gate leaf is a gate leaf whatever the body's wings do.
    /// `DW0453` is a locomotion question, so climbers and fliers are.
    #[test]
    fn the_error_tier_exempts_no_class_and_the_advisory_tier_exempts_by_locomotion() {
        for class in Locomotion::ALL {
            assert!(
                !exempt_from_gate_rule(class),
                "`{}` must never be exempt from the error tier: passing a closed gate is a \
                 collision-and-interaction fact, not a locomotion one",
                class.token()
            );
        }
        assert!(exempt_from_surmount_rule(Locomotion::Climber));
        assert!(exempt_from_surmount_rule(Locomotion::Flier));
        assert!(!exempt_from_surmount_rule(Locomotion::Ground));
        assert!(
            !exempt_from_surmount_rule(Locomotion::Aquatic),
            "the aquatic class is a ledger label and must exempt nothing"
        );
    }

    /// Every hand-listed id must be a real 1.21.11 entity, so a typo or a
    /// species renamed at the next MC pin is a red rather than a class that
    /// silently stopped matching anything.
    #[test]
    fn every_hand_listed_species_exists_in_the_pinned_registry() {
        let ids: std::collections::BTreeSet<String> =
            serde_json::from_str(include_str!("../data/entities-1.21.11.json"))
                .map(|v: Vec<String>| v.into_iter().collect())
                .expect("vendored entity registry is valid JSON");
        for id in CLIMBERS.iter().chain(FLIERS.iter()) {
            assert!(ids.contains(*id), "`{id}` is not a 1.21.11 entity type");
        }
    }

    /// Aquatic membership is vanilla's `#minecraft:aquatic`, read from the
    /// vendored tags — not a list in this file. The three the hand table it
    /// replaced had missed are the assertion that this is really the tag.
    #[test]
    fn aquatic_membership_is_vanillas_own_tag() {
        for id in ["minecraft:turtle", "minecraft:nautilus", "minecraft:squid"] {
            assert_eq!(
                Traversal::of_entity(id).locomotion,
                Locomotion::Aquatic,
                "`{id}` is in #minecraft:aquatic"
            );
        }
    }

    /// The ledger prints one row per class, always — a class added without a row
    /// would make a count silently vanish.
    #[test]
    fn the_ledger_names_every_class() {
        let json = TraversalGate::default().to_json();
        let by_class = json["legs_by_class"].as_object().expect("object");
        assert_eq!(by_class.len(), Locomotion::ALL.len());
        for class in Locomotion::ALL {
            assert!(by_class.contains_key(class.token()), "{class:?}");
        }
        assert_eq!(json["unbound"], serde_json::json!(true));
        assert!(json["reason"].is_string(), "an unbound ledger states why");
    }

    /// The island's north wall, in miniature: a `cobblestone_wall` cell with a
    /// full cube beside it, and a route that hops the cube and drops the far
    /// side. `barrier_course` is the half of the rule that names the line.
    #[test]
    fn a_full_cube_beside_a_wall_is_a_barrier_course() {
        let mut solid = floor(63);
        solid.insert([4, 63, 3]); // the low course
        let w = world(solid, BTreeSet::from([[5, 63, 3]]), BTreeSet::new());
        assert_eq!(barrier_course(&w, [4, 63, 3]), Some([5, 63, 3]));
        // …and a full cube with no barrier in its line is just a block.
        let plain = world(floor(63), BTreeSet::new(), BTreeSet::new());
        assert_eq!(barrier_course(&plain, [4, 62, 3]), None);
    }

    /// A bottom slab beside a fence is floor detail, not a course of the wall.
    #[test]
    fn a_partial_block_is_not_a_course() {
        let mut solid = floor(63);
        solid.insert([4, 63, 3]);
        let mut occ = crate::assembled::Occupancy {
            solid,
            tall: BTreeSet::from([[5, 63, 3]]),
            use_gates: BTreeSet::new(),
            flooded: BTreeSet::new(),
            partial: BTreeMap::new(),
        };
        occ.partial.insert([4, 63, 3], 8);
        let w = World::from_occupancy(occ, crate::nav::Premises::geometry_only());
        assert_eq!(barrier_course(&w, [4, 63, 3]), None);
    }
}
