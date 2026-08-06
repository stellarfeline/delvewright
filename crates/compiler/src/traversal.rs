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
//! do — [`Traversal`], derived from the entity id, because vanilla already draws
//! these lines and they are facts to encode rather than thresholds to invent:
//!
//! * **Ground** (the default, and every body the delves ship today): walks,
//!   steps and jumps. Opens nothing.
//! * **Climber** (`spider`, `cave_spider`): climbs sheer vertical surfaces.
//!   A spider routed over a wall is *correct* and must not be flagged — which is
//!   the whole reason this is a per-body model and not a global rule.
//! * **Flier** (`bat`, `phantom`, `ghast`, `bee`, `allay`, `vex`, …): not bound
//!   to a ground route at all.
//! * **Aquatic** (`squid`, `dolphin`, `guardian`, …): bound to water.
//! * **Gate use** is `false` for every entity in the table. No vanilla mob opens
//!   a fence gate — villagers open *doors* — and a compiler-driven puppet
//!   performs no interaction whatever. It is a field rather than a constant so
//!   the rule reads as the capability claim it is, and so the player's own
//!   routing ([`crate::nav::check_critical_path`]) stays visibly the one caller
//!   that has it.
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
//!   assembled-geometry defect. A climber or a flier is exempt for a different
//!   reason each: a flier is not on the ground route the rule is about; a
//!   climber still cannot open a gate, so it is **not** exempt from this rule.
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

use std::collections::BTreeMap;
use std::fmt::Write as _;

use delvewright_dsl::Diagnostic;

use crate::nav::{ActorMovePlan, MovePlan, World};
use crate::plan::Plan;

/// `DW0452`: a walked leg's route contains a move the body cannot make — today,
/// passing through a closed fence gate with no capability to open one.
pub const DW_TRAVERSAL_IMPOSSIBLE: &str = "DW0452";

/// `DW0453`: a walked leg's route goes **over** a barrier line by stepping onto a
/// full-cube course of it. Advisory: the step is physically legal, and whether
/// the course is a kerb or an enclosure is a content judgement.
pub const DW_BARRIER_SURMOUNTED: &str = "DW0453";

/// How many route steps after a rise still count as "and came down the other
/// side". Four: the island's crossing takes one step up, at most two along the
/// top and one down, and a window this short is what keeps an ordinary ledge the
/// route genuinely uses (climb up, walk, stay up) out of the tier.
pub const SURMOUNT_WINDOW: usize = 4;

/// How a body gets around, derived from its entity id. Vanilla's own lines, not
/// the compiler's — see the module docs for what is deliberately absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Locomotion {
    /// Walks, steps and jumps. Every body the delves ship today.
    Ground,
    /// Climbs sheer vertical surfaces (`spider`, `cave_spider`).
    Climber,
    /// Not bound to a ground route.
    Flier,
    /// Bound to water.
    Aquatic,
}

impl Locomotion {
    /// The stable token this class is reported under in [`TraversalGate`].
    pub fn token(self) -> &'static str {
        match self {
            Locomotion::Ground => "ground",
            Locomotion::Climber => "climber",
            Locomotion::Flier => "flier",
            Locomotion::Aquatic => "aquatic",
        }
    }
}

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
    /// The traversal capabilities of `entity`, by vanilla behaviour.
    ///
    /// Unknown ids fall to [`Locomotion::Ground`] — the conservative direction:
    /// a ground body is the one this proof has rules for, so an unrecognised
    /// entity is *checked*, never silently exempted.
    pub fn of_entity(entity: &str) -> Traversal {
        let id = entity.strip_prefix("minecraft:").unwrap_or(entity);
        let locomotion = match id {
            "spider" | "cave_spider" => Locomotion::Climber,
            "bat" | "phantom" | "ghast" | "happy_ghast" | "bee" | "allay" | "vex" | "blaze"
            | "wither" | "ender_dragon" | "breeze" => Locomotion::Flier,
            "squid" | "glow_squid" | "dolphin" | "guardian" | "elder_guardian" | "cod"
            | "salmon" | "tropical_fish" | "pufferfish" | "tadpole" | "axolotl" => {
                Locomotion::Aquatic
            }
            _ => Locomotion::Ground,
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
/// The capability axis is its own way to bind to nothing: a proof written for
/// walking bodies is *unbound* over every flying or climbing actor in the
/// campaign, and would report green over exactly the bodies it understands
/// least. Hence a count per [`Locomotion`] class, not just a total.
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
}

impl TraversalGate {
    /// Whether this proof matched nothing at all.
    pub fn unbound(&self) -> bool {
        self.legs == 0
    }

    /// The ledger as the `validation/traversal-gate.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        let mut by_class = serde_json::Map::new();
        for class in [
            Locomotion::Ground,
            Locomotion::Climber,
            Locomotion::Flier,
            Locomotion::Aquatic,
        ] {
            by_class.insert(
                class.token().to_string(),
                serde_json::json!(self.legs_by_class.get(class.token()).copied().unwrap_or(0)),
            );
        }
        let mut v = serde_json::json!({
            "legs": self.legs,
            "route_cells": self.route_cells,
            "legs_by_class": by_class,
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

/// A build failure raised by the traversal proof (exit 3, like its neighbours).
#[derive(Debug)]
pub struct TraversalError {
    /// The stable diagnostic code ([`DW_TRAVERSAL_IMPOSSIBLE`]).
    pub code: &'static str,
    /// Human-readable explanation naming the body, the leg, the cell and the
    /// capability the route assumed — plus every further violation, so one build
    /// reports them all.
    pub message: String,
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
) -> Result<(Vec<Diagnostic>, TraversalGate), TraversalError> {
    let mut gate = TraversalGate::default();
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<Diagnostic> = Vec::new();
    for leg in legs(plan, moves, actor_moves) {
        let cap = Traversal::of_entity(&leg.entity);
        gate.legs += 1;
        gate.route_cells += leg.cells.len();
        *gate
            .legs_by_class
            .entry(cap.locomotion.token())
            .or_insert(0) += 1;
        // A flier is not walking the route this proof reasons about; both rules
        // are about ground contact, so neither binds. Recorded in the ledger's
        // class counts rather than silently skipped.
        if cap.locomotion == Locomotion::Flier {
            continue;
        }
        // --- DW0452: a traversal this body cannot perform. -------------------
        if !cap.opens_gates {
            gate.gate_rule_cells += leg.cells.len();
            if let Some(&cell) = leg.cells.iter().find(|&&c| world.is_use_gate(c)) {
                errors.push(gate_violation(&leg, cell));
            }
        }
        // --- DW0453: a barrier line surmounted over a full-cube course. ------
        // A climber goes over things; that is the capability, not the defect.
        if cap.locomotion == Locomotion::Climber {
            continue;
        }
        let mut reported = false;
        for i in 0..leg.cells.len().saturating_sub(1) {
            let (from, onto) = (leg.cells[i], leg.cells[i + 1]);
            if onto[1] <= from[1] {
                continue;
            }
            gate.surmount_rule_rises += 1;
            if reported {
                continue;
            }
            let support = [onto[0], onto[1] - 1, onto[2]];
            let Some(barrier) = barrier_course(world, support) else {
                continue;
            };
            // …and came down the other side. A body that climbs onto a ledge and
            // stays there has used the ledge, not crossed the line.
            let end = (i + 1 + SURMOUNT_WINDOW).min(leg.cells.len());
            if !leg.cells[i + 2..end].iter().any(|c| c[1] <= from[1]) {
                continue;
            }
            warnings.push(surmount_advisory(&leg, from, support, barrier));
            reported = true;
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
        return Err(TraversalError {
            code: DW_TRAVERSAL_IMPOSSIBLE,
            message,
        });
    }
    Ok((warnings, gate))
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
         eye agree, and let the route use the opening."
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
        World::from_occupancy(crate::assembled::Occupancy {
            solid,
            tall,
            use_gates: gates,
            flooded: BTreeSet::new(),
            partial: BTreeMap::new(),
        })
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

    /// Vanilla's own lines, encoded: a spider climbs, a ghast flies, a squid
    /// swims, a sheep walks — and **nothing** opens a fence gate.
    #[test]
    fn capabilities_come_from_the_entity() {
        assert_eq!(
            Traversal::of_entity("minecraft:spider").locomotion,
            Locomotion::Climber
        );
        assert_eq!(
            Traversal::of_entity("minecraft:ghast").locomotion,
            Locomotion::Flier
        );
        assert_eq!(
            Traversal::of_entity("minecraft:dolphin").locomotion,
            Locomotion::Aquatic
        );
        assert_eq!(
            Traversal::of_entity("minecraft:sheep").locomotion,
            Locomotion::Ground
        );
        // An id the table does not know is CHECKED, never exempted.
        assert_eq!(
            Traversal::of_entity("minecraft:mannequin").locomotion,
            Locomotion::Ground
        );
        for e in [
            "minecraft:sheep",
            "minecraft:spider",
            "minecraft:villager",
            "minecraft:ghast",
        ] {
            assert!(
                !Traversal::of_entity(e).opens_gates,
                "no vanilla body opens a fence gate: {e}"
            );
        }
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
        let w = World::from_occupancy(occ);
        assert_eq!(barrier_course(&w, [4, 63, 3]), None);
    }
}
