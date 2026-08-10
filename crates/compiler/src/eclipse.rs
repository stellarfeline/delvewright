//! Body-eclipse proof: an NPC/actor body may not stand in front of an
//! interaction affordance (`DW0359`, build tier) — and no affordance may share a
//! cell with a sealed gate's answer hitboxes (`DW0422`, v0.8; see
//! [`check_seal_collisions`], same box arithmetic, same tier).
//!
//! ## The defect this exists for (owner playtest, island round 7)
//!
//! `npc/polyphemus` — a **warden**-bodied mannequin, 0.9 × 2.9 blocks — stands
//! on `anchor/fire-pit`. So does the `obj/harden` interact affordance, and so
//! does `obj/blind`'s. All three are entities in one cell, and the giant's body
//! is the biggest of them: the player's crosshair ray-picks the `Invulnerable`
//! NPC every time, the interaction entity behind it is never reached, and the
//! harden beat — a required objective — is **unreachable**. A campaign
//! soft-lock, with every other proof green.
//!
//! `DW0350` already forbids one shape of this: a `use` **trigger** sharing an
//! NPC's exact anchor. It is a symbolic DSL-tier check (same anchor name =
//! conflict) and it sees only triggers — an NPC body occluding an *objective's*
//! affordance, or an affordance in the cell next door that a 1.95-wide ravager's
//! shoulder covers, passed silently. This is the geometric statement of the same
//! rule, over the **assembled** world: bodies and affordances are boxes with
//! real sizes at real cells, and a body may not sit on (or immediately in front
//! of) an affordance box.
//!
//! ## The model
//!
//! * **Body.** An NPC's body is its `base_entity`'s standing hitbox
//!   ([`crate::nav::entity_dims`] — the one dims table in the compiler), or the
//!   player-model mannequin's 0.6 × 1.8 when the NPC declares a `skin` (the
//!   compiler then summons `minecraft:mannequin`, not the base entity). An
//!   actor's body is its `entity`, likewise. Horizontally the box is centred on
//!   its anchor cell's centre and is `width` across; vertically it rises
//!   `height` from the anchor cell's floor. `deferred` NPCs count: they arrive
//!   at the very same anchor later, and a soft-lock that starts at minute 30 is
//!   still a soft-lock.
//! * **Affordance.** Every right-click target the compiler summons is a
//!   `minecraft:interaction` with `width:1.0f,height:2.0f` at a cell centre, so
//!   its box is *exactly* the anchor cell's column, two blocks tall. Five
//!   sources, one shape: `interact` objectives, `use`/`strike` env triggers,
//!   `bonfire` rest affordances, `shortcut` unlock affordances, and trap /
//!   `timed-gate` `disarm` affordances.
//! * **Eclipse — error.** The two boxes overlap in all three axes. The
//!   affordance is inside the body; nothing the player can do with a crosshair
//!   reaches it.
//! * **Crowding — warning.** They do not overlap, but the horizontal gap
//!   between them is ≤ [`CROWD_GAP`] (1 block) and their vertical spans still
//!   overlap. Whether an adjacent body actually shadows the affordance depends
//!   on the approach angle the player happens to take, which the compiler
//!   cannot know — so it reports the measurement and leaves the verdict to the
//!   owner's QA hour (`Severity::Warning` is exactly for this). Two blocks of
//!   clearance is silent.
//!
//! ## Two exemptions, both about not inventing certainty
//!
//! **A `strike` trigger on an NPC's own anchor** summons no entity of its own:
//! `emit::strike_trigger_tags_at` puts the trigger's tag on the NPC's hitbox
//! instead, and `env_trigger_setup` suppresses the standalone summon. There is
//! no second entity to eclipse, so there is nothing here to report — the giant's
//! un-strikeable *body* is a different defect with a different fix (routing the
//! `attack` event through the NPC's own interaction), not a placement error.
//!
//! **A body the campaign ever moves** (`move-npc` / `move-actor`, at any nesting
//! depth) is skipped entirely. This is the *parked-body* rule: it reads a
//! declared standing anchor, which for a walker is only a starting mark. The
//! canonical shape is an NPC who stands on a lever from world init and walks
//! away on the very beat that arms it — a body and an affordance that coexist
//! for a few ticks and never block anything. Deciding those needs a timeline
//! ("is the body still there when the affordance goes live?"), and the compiler
//! will not guess one; [`crate::continuity`] takes the same no-false-certainty
//! stance about the same NPC lifecycle. The cost is a known blind spot — an NPC
//! *walked onto* an affordance and left there — which wants a `move-npc`
//! destination rule of its own, not a guess bolted onto this one.
//!
//! ## Prescription, and the fix that is never prescribed
//!
//! Move the NPC's anchor, or move the interaction's anchor. **Never** make the
//! NPC intangible/non-pickable to let clicks pass through it: an NPC the party
//! cannot click is an NPC they cannot talk to, which trades a dead objective for
//! a dead character. The affordance and the body each need their own cell.

use std::collections::BTreeSet;

use delvewright_dsl::{Campaign, Diagnostic, Objective, QuestEffect, TriggerOn};

use crate::nav::entity_dims;
use crate::plan::Plan;

/// `DW0359`: an NPC or actor body stands on (error) or immediately in front of
/// (warning) an interaction affordance, so the player's crosshair reaches the
/// body instead of the affordance.
pub const DW_BODY_ECLIPSE: &str = "DW0359";

/// `DW0422`: a **seal's answer hitbox** shares space with another compiler-owned
/// interaction affordance (DSL v0.8, task #142).
///
/// A `close-gate` arms one `minecraft:interaction` per clickable cell of the
/// sealed region so the wall can answer a right-click. Any other affordance whose
/// own 1.0 × 2.0 box overlaps one of those cells is in an exact ray-pick contest
/// with it, and the client resolves such a contest by iteration order — so one of
/// the two silently stops receiving clicks, which is precisely the defect
/// (`DESIGN.md`, island round 13) that made a second hitbox on the boulder
/// unshippable. Triggers anchored **on the gate itself** are not a collision: they
/// ride the seal's hitboxes and summon nothing (`emit::env_trigger_setup`), the
/// same merge `strike`-on-an-NPC's-anchor has used since round 6.
pub const DW_SEAL_HITBOX_COLLISION: &str = "DW0422";

/// A build failure raised by the eclipse proof (mapped to exit 3, like the
/// `nav`/`edit` build errors it sits beside).
#[derive(Debug)]
pub struct EclipseError {
    /// The stable diagnostic code ([`DW_BODY_ECLIPSE`]).
    pub code: &'static str,
    /// Human-readable explanation, naming both entities and both cells.
    pub message: String,
}

/// Every affordance the compiler summons is `minecraft:interaction` with
/// `width:1.0f` — exactly one cell across, centred on the cell.
const AFFORDANCE_WIDTH: f64 = 1.0;

/// …and `height:2.0f`, rising from the cell floor.
const AFFORDANCE_HEIGHT: f64 = 2.0;

/// The horizontal clearance below which a neighbouring body is close enough to
/// shadow the affordance from a plausible approach angle (warning tier). One
/// block: at two blocks of clearance the body is out of the crosshair cone for
/// any player standing at the affordance.
const CROWD_GAP: f64 = 1.0;

/// A standing body: an NPC or an actor at its declared anchor.
struct Body {
    /// `npc` or `actor`, for the message.
    kind: &'static str,
    /// The declaring id (`npc/polyphemus`).
    id: String,
    /// The entity id whose hitbox the body wears (`minecraft:warden`, or
    /// `minecraft:mannequin` for a skinned NPC).
    entity: String,
    /// The declared standing anchor.
    anchor: String,
    /// The resolved anchor cell (feet).
    pos: [i32; 3],
    /// JSON pointer at the declaration, for the diagnostic path.
    path: String,
}

/// An interaction affordance: one `minecraft:interaction` entity at a cell.
pub(crate) struct Affordance {
    /// What declares it (`interact objective`, `trigger`, …), for the message.
    pub(crate) kind: &'static str,
    /// The declaring id (`obj/harden`).
    pub(crate) id: String,
    /// The anchor it sits on.
    pub(crate) anchor: String,
    /// The resolved cell.
    pub(crate) pos: [i32; 3],
}

/// An axis-aligned interval `[lo, hi)`.
#[derive(Clone, Copy)]
struct Span {
    lo: f64,
    hi: f64,
}

impl Span {
    /// Positive-length overlap (touching faces do not overlap).
    fn overlaps(self, other: Span) -> bool {
        self.lo < other.hi && other.lo < self.hi
    }

    /// The empty gap between two spans (0 when they touch or overlap).
    fn gap(self, other: Span) -> f64 {
        (other.lo - self.hi).max(self.lo - other.hi).max(0.0)
    }
}

/// A body's box: the entity hitbox centred on its anchor cell's centre, rising
/// from the cell floor.
fn body_box(pos: [i32; 3], width: f64, height: f64) -> [Span; 3] {
    let half = width / 2.0;
    [
        Span {
            lo: pos[0] as f64 + 0.5 - half,
            hi: pos[0] as f64 + 0.5 + half,
        },
        Span {
            lo: pos[1] as f64,
            hi: pos[1] as f64 + height,
        },
        Span {
            lo: pos[2] as f64 + 0.5 - half,
            hi: pos[2] as f64 + 0.5 + half,
        },
    ]
}

/// The affordance's box — a 1.0 × 2.0 × 1.0 interaction entity at a cell centre,
/// i.e. exactly that cell's column, two blocks tall.
fn affordance_box(pos: [i32; 3]) -> [Span; 3] {
    body_box(pos, AFFORDANCE_WIDTH, AFFORDANCE_HEIGHT)
}

/// The verdict for one (body, affordance) pair.
enum Verdict {
    /// Boxes overlap in all three axes: the affordance is unreachable.
    Eclipsed,
    /// Boxes are apart but within [`CROWD_GAP`] horizontally, spans overlapping
    /// vertically. Carries the measured horizontal gap.
    Crowded(f64),
    /// Clear.
    Clear,
}

/// Compare one body box against one affordance box.
fn verdict(body: [Span; 3], aff: [Span; 3]) -> Verdict {
    if !body[1].overlaps(aff[1]) {
        // No shared vertical band: the body is entirely above or below the
        // affordance and cannot be in front of it at all.
        return Verdict::Clear;
    }
    if body[0].overlaps(aff[0]) && body[2].overlaps(aff[2]) {
        return Verdict::Eclipsed;
    }
    // Chebyshev separation: "one block away" means one block on the worse axis,
    // which is how a player reads adjacency in a block world.
    let gap = body[0].gap(aff[0]).max(body[2].gap(aff[2]));
    if gap <= CROWD_GAP {
        Verdict::Crowded(gap)
    } else {
        Verdict::Clear
    }
}

/// Every npc/actor id the campaign ever **moves**, at any nesting depth — the
/// bodies whose declared anchor is only a starting mark (see the module docs).
fn walkers(c: &Campaign) -> BTreeSet<&str> {
    // Every root, inherited from the single enumeration. This walk had grown the
    // dialogue `on_respawn` root by hand and never got `traps[].payload` — the one
    // walker on the sweep that was blind to R4 alone, which is exactly what
    // enumerating roots by hand produces: each copy misses a different one.
    let mut out = BTreeSet::new();
    delvewright_dsl::for_each_campaign_effect(c, &mut |_path, _site, e| match e {
        QuestEffect::MoveNpc { npc, .. } => {
            out.insert(npc.as_str());
        }
        QuestEffect::MoveActor { actor, .. } => {
            out.insert(actor.as_str());
        }
        _ => {}
    });
    out
}

/// Every standing body in the campaign, in declaration order (NPCs then actors).
/// A body whose anchor does not resolve is skipped: `DW0345`/`DW0360`/`DW0325`
/// own unresolved anchors, and guessing a position here would report a geometry
/// defect for what is really a dangling reference.
fn bodies(plan: &Plan) -> Vec<Body> {
    let c = plan.campaign;
    let walkers = walkers(c);
    let mut out = Vec::new();
    for (i, n) in c.npcs.content.npcs.iter().enumerate() {
        if walkers.contains(n.id.as_str()) {
            continue;
        }
        let area = n.area.as_str();
        let Some(pos) = plan
            .point(area, n.anchor.as_str())
            .or_else(|| plan.point_any(n.anchor.as_str()))
        else {
            continue;
        };
        // A skinned NPC is summoned as `minecraft:mannequin` — the player model,
        // not the declared base entity (see `emit::npc_summon_commands`). Model
        // what ships, not what is declared.
        let entity = crate::nav::npc_body_entity(n);
        out.push(Body {
            kind: "npc",
            id: n.id.as_str().to_string(),
            entity,
            anchor: n.anchor.as_str().to_string(),
            pos,
            path: format!("/content/npcs/{i}"),
        });
    }
    for (i, a) in c.quests.content.actors.iter().enumerate() {
        if walkers.contains(a.id.as_str()) {
            continue;
        }
        let Some(pos) = plan.point_any(a.anchor.as_str()) else {
            continue;
        };
        let entity = crate::nav::actor_body_entity(a);
        out.push(Body {
            kind: "actor",
            id: a.id.as_str().to_string(),
            entity,
            anchor: a.anchor.as_str().to_string(),
            pos,
            path: format!("/content/actors/{i}"),
        });
    }
    out
}

/// Every interaction affordance the compiler will summon, in a deterministic
/// order (objectives, triggers, bonfires, shortcut unlocks, trap disarms).
///
/// The single authority on the set, read by `DW0359` here and by `DW0542`
/// ([`crate::teleport`]) — so an affordance added to the engine enters both
/// proofs by existing rather than by being remembered twice.
pub(crate) fn affordances(plan: &Plan) -> Vec<Affordance> {
    let c = plan.campaign;
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            let Objective::Interact { id, anchor, .. } = o else {
                continue;
            };
            // Mirrors `emit::activation_commands`: an interact objective's
            // affordance is resolved within its quest's area.
            let Some(pos) = plan.point(area, anchor.as_str()) else {
                continue;
            };
            out.push(Affordance {
                kind: "interact objective",
                id: id.as_str().to_string(),
                anchor: anchor.as_str().to_string(),
                pos,
            });
        }
    }
    for t in &c.quests.content.triggers {
        // `approach` triggers are a per-tick radius test, not an entity.
        if matches!(t.on, TriggerOn::Approach { .. }) {
            continue;
        }
        // `strike-npc` (DSL v0.6) has no cell at all — it rides the NPC's own
        // hitbox by construction, so eclipse is its *mechanism*, not a defect.
        let Some(at) = t.at_anchor() else {
            continue;
        };
        // The same shape spelled the old way: a `strike` trigger on an NPC's own
        // anchor rides that NPC's hitbox and summons nothing (see the module
        // docs). No second entity, nothing to eclipse.
        if matches!(t.on, TriggerOn::Strike) && npc_stands_at(plan, at) {
            continue;
        }
        // …and the general form of the same merge (task #50): wherever a
        // compiler-owned interaction set already covers the anchor — a
        // `close-gate` seal, a sealed shortcut door — the trigger rides it and
        // summons nothing. Read from `crate::pressable`, the same authority the
        // emitter uses, so the two can never disagree about whether a body exists.
        if matches!(
            crate::pressable::body_at(plan, at),
            crate::pressable::Body::Rides { .. }
        ) {
            continue;
        }
        let Some(pos) = plan.point_any(at) else {
            continue;
        };
        out.push(Affordance {
            kind: "trigger",
            id: t.id.as_str().to_string(),
            anchor: at.to_string(),
            pos,
        });
    }
    for bf in plan.bonfires() {
        out.push(Affordance {
            kind: "bonfire",
            id: format!("bonfire #{}", bf.index),
            anchor: bf.anchor.clone(),
            pos: bf.pos,
        });
    }
    for sc in &plan.shortcuts {
        out.push(Affordance {
            kind: "shortcut unlock",
            id: sc.id.clone(),
            anchor: sc.unlock_anchor.clone(),
            pos: sc.unlock,
        });
    }
    for tr in &plan.traps {
        if let Some(d) = &tr.disarm {
            out.push(Affordance {
                kind: "trap disarm",
                id: tr.id.clone(),
                anchor: d.via_anchor.clone(),
                pos: d.via_cell,
            });
        }
    }
    for g in &plan.timed_gates {
        if let Some(d) = &g.disarm {
            out.push(Affordance {
                kind: "timed-gate disarm",
                id: g.id.clone(),
                anchor: d.via_anchor.clone(),
                pos: d.via_cell,
            });
        }
    }
    out
}

/// Whether a stage-2 NPC declares `anchor` as its standing anchor.
fn npc_stands_at(plan: &Plan, anchor: &str) -> bool {
    plan.campaign
        .npcs
        .content
        .npcs
        .iter()
        .any(|n| n.anchor.as_str() == anchor)
}

/// Prove no body eclipses an interaction affordance (`DW0359`).
///
/// Returns the crowding **warnings** on success; the first eclipse is an error
/// (build tier, exit 3) and stops the build. Both are produced in
/// body-then-affordance declaration order, so the report is deterministic.
///
/// Empty for every campaign whose bodies and affordances keep two blocks apart —
/// which is every campaign that already compiled clean, so output stays
/// byte-identical.
pub fn check_body_eclipse(plan: &Plan) -> Result<Vec<Diagnostic>, EclipseError> {
    let bodies = bodies(plan);
    let affordances = affordances(plan);
    let mut warnings = Vec::new();
    for b in &bodies {
        let (w, h) = entity_dims(&b.entity);
        let bbox = body_box(b.pos, w, h);
        for a in &affordances {
            match verdict(bbox, affordance_box(a.pos)) {
                Verdict::Clear => {}
                Verdict::Eclipsed => return Err(eclipse_error(b, a, w, h)),
                Verdict::Crowded(gap) => warnings.push(crowding_warning(b, a, w, h, gap)),
            }
        }
    }
    Ok(warnings)
}

/// The box one seal-answer hitbox occupies: its cell, grown by
/// [`crate::emit::SEAL_MARGIN`] on every side so the entity is strictly nearer
/// the player than the sealed block it stands in (an exactly-coincident box loses
/// the client's ray-pick to the block, and the seal answers with silence).
fn seal_box(cell: [i32; 3]) -> [Span; 3] {
    let m = crate::emit::SEAL_MARGIN;
    [
        Span {
            lo: cell[0] as f64 - m,
            hi: cell[0] as f64 + 1.0 + m,
        },
        Span {
            lo: cell[1] as f64 - m,
            hi: cell[1] as f64 + 1.0 + m,
        },
        Span {
            lo: cell[2] as f64 - m,
            hi: cell[2] as f64 + 1.0 + m,
        },
    ]
}

/// Prove no other affordance contests a seal's answer hitboxes (`DW0422`).
///
/// Build tier (exit 3), pure box arithmetic over resolved cells — it runs beside
/// [`check_body_eclipse`], before any occupancy model. Empty (and byte-identical)
/// for a campaign that seals no gate, and for every campaign whose other
/// affordances simply are not inside a sealed region.
pub fn check_seal_collisions(plan: &Plan) -> Result<(), EclipseError> {
    let mut affordances = affordances(plan);
    // An NPC's dialogue hitbox is an affordance too — the one whose loss the
    // round-6 island proved (Polyphemus untalkable after the boulder seal, because
    // a second entity in his cell took every right-click). It is not in
    // `affordances()` because `DW0359` models the NPC as a *body* there; here the
    // contest is hitbox-vs-hitbox, so the hitbox is what counts. Walkers are
    // skipped for the same reason `DW0359` skips them: a declared anchor is only a
    // starting mark, and the compiler will not guess a timeline (`bodies` applies
    // the parked-body rule).
    affordances.extend(
        bodies(plan)
            .into_iter()
            .filter(|b| b.kind == "npc")
            .map(|b| Affordance {
                kind: "npc dialogue hitbox",
                id: b.id,
                anchor: b.anchor,
                pos: b.pos,
            }),
    );
    for s in &plan.seal_hints {
        for cell in s.shell_cells() {
            let sbox = seal_box(cell);
            for a in &affordances {
                let abox = affordance_box(a.pos);
                if (0..3).all(|i| sbox[i].overlaps(abox[i])) {
                    return Err(seal_collision_error(s, cell, a));
                }
            }
        }
    }
    Ok(())
}

/// The `DW0422` error: another affordance stands inside a sealed region.
fn seal_collision_error(
    s: &crate::plan::SealHintPlan,
    cell: [i32; 3],
    a: &Affordance,
) -> EclipseError {
    EclipseError {
        code: DW_SEAL_HITBOX_COLLISION,
        message: format!(
            "the {} `{}` at `{}` {:?} shares space with the seal of gate anchor `{}`, which arms \
             an answer hitbox at {:?}. Both are `minecraft:interaction` boxes in the same cell, so \
             the client's entity ray-pick is an exact tie and resolves by iteration order — one of \
             the two silently stops receiving clicks, and which one is not decidable from the \
             campaign. Prescription: move the affordance out of the sealed region, or — when the \
             thing being clicked really IS the sealed gate — anchor the trigger on the gate anchor \
             `{}` itself, which makes it ride the seal's own hitboxes instead of summoning a \
             second one.",
            a.kind, a.id, a.anchor, a.pos, s.anchor, cell, s.anchor,
        ),
    }
}

/// How a body is described in a message: ``npc `npc/polyphemus`
/// (minecraft:warden, 0.9 × 2.9 blocks)``.
fn describe(b: &Body, w: f64, h: f64) -> String {
    format!("{} `{}` ({}, {w} × {h} blocks)", b.kind, b.id, b.entity)
}

/// The `DW0359` error: the body sits on the affordance.
fn eclipse_error(b: &Body, a: &Affordance, w: f64, h: f64) -> EclipseError {
    EclipseError {
        code: DW_BODY_ECLIPSE,
        message: format!(
            "{} stands at `{}` {:?} and ECLIPSES the {} `{}`'s affordance at `{}` {:?}: the body's \
             hitbox overlaps the interaction entity's own 1.0 × 2.0 box, so a player's crosshair \
             ray-picks the invulnerable body and the affordance can never be clicked — the \
             interaction is unreachable and, if it is required, the delve soft-locks there. \
             Prescription: move the {}'s anchor, or move the interaction's anchor, so the two are \
             at least 2 blocks apart. Do NOT make the body intangible or non-pickable to let \
             clicks through: a body the party cannot click is a character they cannot talk to, \
             which trades a dead objective for a dead NPC.",
            describe(b, w, h),
            b.anchor,
            b.pos,
            a.kind,
            a.id,
            a.anchor,
            a.pos,
            b.kind,
        ),
    }
}

/// The `DW0359` warning: the body is beside the affordance, close enough to
/// shadow it from some approach angles.
fn crowding_warning(b: &Body, a: &Affordance, w: f64, h: f64, gap: f64) -> Diagnostic {
    Diagnostic::warning(
        DW_BODY_ECLIPSE,
        stage_of(b),
        b.path.clone(),
        format!(
            "{} stands at `{}` {:?}, only {gap:.2} blocks clear of the {} `{}`'s affordance at \
             `{}` {:?}, and their hitboxes share a vertical band. From an approach angle that puts \
             the body between the player and the affordance, the crosshair picks the body and the \
             click is lost. Advisory, not an error: which angles a player actually takes is not \
             something the compiler can know. Prescription: move the {}'s anchor or the \
             interaction's anchor 2+ blocks apart, or confirm the approach in playtest — never by \
             making the body intangible.",
            describe(b, w, h),
            b.anchor,
            b.pos,
            a.kind,
            a.id,
            a.anchor,
            a.pos,
            b.kind,
        ),
    )
}

/// The DSL stage a body is declared in — `npcs` for stage-2 NPCs, `quests` for
/// stage-5 actors.
fn stage_of(b: &Body) -> &'static str {
    match b.kind {
        "actor" => "quests",
        _ => "npcs",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The island's shape: a 0.9 × 2.9 warden and an affordance in one cell.
    #[test]
    fn warden_on_the_affordance_cell_is_eclipsed() {
        let b = body_box([10, 64, 10], 0.9, 2.9);
        assert!(matches!(
            verdict(b, affordance_box([10, 64, 10])),
            Verdict::Eclipsed
        ));
    }

    #[test]
    fn warden_one_cell_over_is_crowding_not_eclipse() {
        let b = body_box([10, 64, 10], 0.9, 2.9);
        let Verdict::Crowded(gap) = verdict(b, affordance_box([11, 64, 10])) else {
            panic!("adjacent cell must be the warning tier");
        };
        // 0.9-wide body: 0.05 of the cell on each side is empty.
        assert!((gap - 0.05).abs() < 1e-9, "measured gap {gap}");
    }

    #[test]
    fn two_blocks_of_clearance_is_silent() {
        let b = body_box([10, 64, 10], 0.9, 2.9);
        assert!(matches!(
            verdict(b, affordance_box([12, 64, 10])),
            Verdict::Clear
        ));
    }

    /// A 1.95-wide ravager reaches into the next cell — the case the symbolic
    /// same-anchor check (`DW0350`) can never see.
    #[test]
    fn a_ravager_eclipses_the_neighbouring_cell() {
        let (w, h) = entity_dims("minecraft:ravager");
        let b = body_box([10, 64, 10], w, h);
        assert!(matches!(
            verdict(b, affordance_box([11, 64, 10])),
            Verdict::Eclipsed
        ));
    }

    /// A body on a different floor shares no vertical band and is not in front
    /// of anything — but a 2.9-tall warden one storey down still reaches up.
    #[test]
    fn vertical_bands_decide_whether_a_body_is_in_front() {
        let b = body_box([10, 64, 10], 0.6, 1.95);
        assert!(matches!(
            verdict(b, affordance_box([10, 60, 10])),
            Verdict::Clear
        ));
        let tall = body_box([10, 62, 10], 0.9, 2.9);
        assert!(matches!(
            verdict(tall, affordance_box([10, 64, 10])),
            Verdict::Eclipsed
        ));
    }

    /// The code is the one the reference documents.
    #[test]
    fn code_is_dw0359() {
        assert_eq!(DW_BODY_ECLIPSE, "DW0359");
    }
}
