//! Body-clearance proof: an NPC or actor body may not occupy the same space as
//! block geometry (`DW0450` error, `DW0451` advisory).
//!
//! ## The defect this exists for (owner playtest, island rounds 8/10/11)
//!
//! Three rounds, one shape: a large-bodied puppet is **visibly inside solid
//! rock** on the live server while every compile-time proof is green.
//!
//! Round 11's instance is the cleanest statement of it.
//! `actor/polyphemus-walker` — a `minecraft:warden`, 0.9 × 2.9 blocks — is
//! `spawn-actor`ed at `anchor/mouth-side`. That anchor resolves to world cell
//! `[6, 69, -45]`, and in the final assembled world `[6, 69, -45]`,
//! `[6, 70, -45]` and `[6, 71, -45]` are all `minecraft:cobblestone`: the entire
//! three-cell column the giant's body needs is the cliff face beside the cave
//! mouth. The emitted command is literally
//! `summon minecraft:warden 6.5 69.0 -44.5 …` — into the rock.
//!
//! **Why nothing caught it** is an asymmetry the compiler had never stated:
//!
//! * A **walked** destination is proven. `move-npc` / `move-actor` snap their
//!   endpoints to a standable cell ([`crate::nav::World::snap`], `SNAP_RADIUS`)
//!   and A* only ever steps through passable cells, so a walk *cannot* end
//!   inside a wall.
//! * A **placed** body is not. [`crate::nav::check_actor_placement`] (`DW0325`)
//!   proves an actor's spawn anchor *resolves to a cell* and nothing more; NPC
//!   anchors get the same treatment. `summon` does no snapping, so whatever cell
//!   the anchor names is where the body appears — wall or not.
//!
//! `DW0359` ([`crate::eclipse`]) is the neighbouring proof and does not cover
//! this: it compares a body box against *affordance* boxes, never against
//! blocks, and it deliberately skips any body the campaign moves.
//!
//! So the rule this module adds: **every body volume the delve ever ships —
//! standing at its anchor, and at every tick of every walk — must be clear of
//! block geometry.**
//!
//! ## The model
//!
//! * **Body.** The entity's standing hitbox from [`crate::nav::entity_dims`] —
//!   the one dims table in the compiler, shared with `DW0359` and with actor
//!   footprint routing. Horizontally the AABB is `width` across, centred on the
//!   position; vertically it rises `height` from the feet.
//! * **Positions.** Two kinds, both real: the **anchor** a body is summoned on
//!   (NPCs incl. `deferred`, actors incl. spawn-and-unleash), and **every
//!   emitted waypoint** of every planned `move-npc` / `move-actor` leg — the
//!   exact per-tick `tp` coordinates the datapack ships, at any nesting depth,
//!   because [`crate::nav::plan_moves`] / [`crate::nav::plan_actor_moves`]
//!   already flattened `sequence` legs into plans.
//! * **World.** The same assembled model every other geometry proof reads
//!   (`DW0311`/`DW0359`/`DW0410`): gravity-settled, socket-sealed, stage-7
//!   edits replayed, relight fixtures included, gate regions in their world-init
//!   state.
//! * **Blocks.** A cell's collision volume, not its cell: a full cube occupies
//!   `y ..= y+1`, a bottom slab `y ..= y+0.5`, a `dirt_path` `y ..= y+15/16`
//!   ([`crate::nav::World::solid_top_16`]). Water is not geometry — a body in
//!   water is wading, not clipping — and is excluded.
//!
//! ## Two tiers, and why the line is where it is
//!
//! * **`DW0450` — error, hitbox vs full-cube solid.** The body's collision box
//!   overlaps a solid block's collision box with positive volume. There is no
//!   tolerance question here: the entity is inside the wall, the server will
//!   either leave it embedded (`NoGravity` puppets do not settle out) or eject
//!   it somewhere the author never staged. Build tier, like every other
//!   assembled-geometry defect.
//! * **`DW0451` — warning, the two cases the compiler can measure but not
//!   judge.**
//!   1. **Model overhang**, for a body **at rest** only. The hitbox is clear,
//!      but a solid block lies within [`MODEL_MARGIN`] of it horizontally.
//!      Vanilla mob models routinely render past their collision box — a
//!      warden's shoulders and arms, an iron golem's, a ravager's horns — so a
//!      body flush against a wall *looks* embedded even though nothing
//!      physically overlaps. How far a given model overhangs is not in any data
//!      the compiler has (it is geometry in the client's entity models, not in
//!      the block/entity registries), so this is a **measurement with a named
//!      margin**, never a verdict. Walked legs are exempt: a walker in a
//!      one-block corridor is a fraction of a block from both walls by
//!      construction, so flagging legs would report the map's own dimensions
//!      once per leg and bury the cases that mean something.
//!   2. **1.5-tall barriers.** A fence, wall or closed fence gate cell falls
//!      inside the body volume. Those blocks fill their cell for pathing but are
//!      a narrow post/panel in reality, so whether the body actually
//!      interpenetrates depends on sub-block shape the occupancy model does not
//!      carry. Reported with its cell, judged by the owner's QA hour.
//!
//! A body already proven inside the rock is reported once, at the error tier: it
//! does not also need to be told that its shoulders stick out.
//!
//! ## Prescription
//!
//! Move the anchor to a cell with real clearance (the diagnostic names how much
//! the body needs), or, for a walk, give the leg a corridor the body fits.
//! **Never** shrink the body to fit: `move-npc` plans on the *player* footprint
//! by construction, so a warden-bodied NPC walked down a 2-high corridor is a
//! body the route was never sized for — the fix is the route or the body, never
//! a smaller number in the dims table.

use crate::failure::Failure;
use std::fmt::Write as _;

use delvewright_dsl::Diagnostic;

use crate::nav::{ActorMovePlan, BARRIER_HEIGHT, MovePlan, World, entity_dims};
use crate::plan::Plan;
use delvewright_dsl::{DwCode, ExitTier};

/// `DW0450`: a body volume overlaps solid block geometry — the entity is inside
/// a wall, at its spawn anchor or at some tick of a walked leg.
pub const DW_BODY_CLEARANCE: DwCode = DwCode::every_version("DW0450", ExitTier::Build);

/// `DW0451`: a body volume is clear of solids but its rendered model overhangs
/// into one ([`MODEL_MARGIN`]), or it contains a 1.5-tall fence/wall/gate cell.
/// Advisory: both are measurements the compiler can state honestly and cannot
/// adjudicate.
pub const DW_BODY_CLEARANCE_ADVISORY: DwCode = DwCode::every_version("DW0451", ExitTier::Build);

/// How far past its collision box a vanilla mob model may visibly render, per
/// horizontal side, in blocks.
///
/// Entity models are authored in sixteenths and the limbs of the big mobs — the
/// warden's arms, the iron golem's, a ravager's horns, a sheep's wool — sit a
/// few pixels outside the hitbox in their idle pose. 0.2 blocks (3.2 pixels) is
/// that allowance, and it is deliberately ONE number rather than a per-entity
/// table: real per-model extents are client render geometry, not registry data,
/// so a table would be invented precision. It only ever feeds the **warning**
/// tier ([`DW_BODY_CLEARANCE_ADVISORY`]) — nothing fails a build on an estimate.
///
/// The value is also what makes the tier *discriminating* rather than noisy, and
/// that is not a coincidence. A body leaves `(1 - width)/2` of its own cell free
/// on each side, so this margin fires exactly when a body is too wide to keep
/// 0.2 blocks of its cell between itself and the neighbour: a 0.9-wide warden or
/// sheep (0.05 free) is flush against any wall beside it and will overhang into
/// it; a 0.6-wide player-model humanoid (0.2 free) is not, and an NPC standing
/// against a wall — the most ordinary staging there is — stays silent. Measured
/// on the island: 0.25 flagged 38 bodies including every crew mannequin, 0.2
/// flags only the genuinely flush ones.
pub const MODEL_MARGIN: f64 = 0.2;

/// Where a body volume comes from, for the diagnostic text.
#[derive(Clone)]
enum Where {
    /// The body stands here from its summon: a stage-2 NPC's `anchor`, or a
    /// stage-5 actor's spawn `anchor`.
    Anchor {
        /// `npc` or `actor`.
        kind: &'static str,
        /// The declared anchor id.
        anchor: String,
    },
    /// The body passes through here at tick `tick` of the walk to `to_anchor`.
    Leg {
        /// `move-npc` or `move-actor`.
        verb: &'static str,
        /// The leg's destination anchor id.
        to_anchor: String,
        /// The waypoint index (= tick) inside the leg.
        tick: usize,
        /// Total waypoints in the leg, so "tick 0 of 289" reads as a position.
        ticks: usize,
    },
}

/// One body volume to prove clear: an entity of known dims at a known position.
#[derive(Clone)]
struct Volume {
    /// The declaring id (`actor/polyphemus-walker`).
    id: String,
    /// The entity whose hitbox the body wears (mannequin for a skinned body).
    entity: String,
    /// Feet position: `[centre x, feet y, centre z]` in world coordinates —
    /// exactly what the emitted `summon` / `tp` carries.
    pos: [f64; 3],
    /// JSON pointer at the declaration, for a warning's diagnostic path.
    path: String,
    /// The DSL stage the declaration lives in (`npcs` / `quests`).
    stage: &'static str,
    /// What put the body here.
    at: Where,
}

/// What a body volume was found overlapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    /// A full-cube-class solid block's collision volume — error tier.
    Solid,
    /// A 1.5-tall fence / wall / closed fence gate cell — advisory tier.
    Barrier,
    /// No hitbox overlap, but the rendered model reaches into a solid — advisory.
    ModelOverhang,
}

/// An axis-aligned interval, half-open in the sense that touching faces do not
/// overlap (a body flush against a wall is *not* inside it).
#[derive(Clone, Copy)]
struct Span {
    lo: f64,
    hi: f64,
}

impl Span {
    fn overlaps(self, other: Span) -> bool {
        self.lo < other.hi && other.lo < self.hi
    }

    /// The integer cell indices this span touches, as an inclusive range.
    fn cells(self) -> std::ops::RangeInclusive<i32> {
        self.lo.floor() as i32..=((self.hi - 1e-9).floor() as i32)
    }
}

/// A body AABB: `width` across, centred horizontally on `pos`, rising `height`
/// from `pos`'s feet level.
fn body_box(pos: [f64; 3], width: f64, height: f64) -> [Span; 3] {
    let half = width / 2.0;
    [
        Span {
            lo: pos[0] - half,
            hi: pos[0] + half,
        },
        Span {
            lo: pos[1],
            hi: pos[1] + height,
        },
        Span {
            lo: pos[2] - half,
            hi: pos[2] + half,
        },
    ]
}

/// The first block volume this body box overlaps, scanning cells in a fixed
/// `(x, y, z)` ascending order (ADR-0006). `solids_only` restricts the scan to
/// full-cube-class solids, which is how the model-overhang probe avoids
/// double-reporting a barrier it already saw.
fn first_hit(world: &World, b: [Span; 3], solids_only: bool) -> Option<([i32; 3], Hit)> {
    for x in b[0].cells() {
        for y in b[1].cells() {
            for z in b[2].cells() {
                let c = [x, y, z];
                if let Some(top) = world.solid_top_16(c) {
                    let block = Span {
                        lo: y as f64,
                        hi: y as f64 + f64::from(top) / 16.0,
                    };
                    if b[1].overlaps(block) {
                        return Some((c, Hit::Solid));
                    }
                } else if !solids_only && world.is_barrier(c) {
                    let block = Span {
                        lo: y as f64,
                        hi: y as f64 + BARRIER_HEIGHT,
                    };
                    if b[1].overlaps(block) {
                        return Some((c, Hit::Barrier));
                    }
                }
            }
        }
    }
    None
}

/// The verdict for one body volume: its worst finding, or `None` when clear.
///
/// The model-overhang probe runs for a body **at rest** only. A body at rest is
/// a composed pose the party stands and looks at, so "its shoulder is in the
/// wall" is a real note about the shot. A body mid-stride is not: `move-npc`
/// and `move-actor` legs route down corridors, and a walker passing through a
/// one-block-wide corridor is within a fraction of a block of both walls by
/// construction — flagging that would report the map's own dimensions once per
/// leg and drown the tier that matters. The hard hitbox rule (`DW0450`) applies
/// to every tick regardless: a body may never be *inside* the wall, moving or
/// not.
fn verdict(world: &World, v: &Volume, width: f64, height: f64) -> Option<([i32; 3], Hit)> {
    let hitbox = body_box(v.pos, width, height);
    if let Some(hit) = first_hit(world, hitbox, false) {
        return Some(hit);
    }
    if !matches!(v.at, Where::Anchor { .. }) {
        return None;
    }
    // Hitbox clear: does the *rendered* model still reach into rock? Widened
    // horizontally only — models overhang at the limbs, not below the feet, and
    // an entity's model height is its hitbox height for every mob in the table.
    let model = body_box(v.pos, width + 2.0 * MODEL_MARGIN, height);
    first_hit(world, model, true).map(|(c, _)| (c, Hit::ModelOverhang))
}

/// Whether the campaign declares any body at all — the cheap gate that decides
/// whether the assembled world is worth building for this proof. False only for
/// a campaign with neither an NPC nor an actor, which then keeps its exact
/// pre-existing build path (and byte-identical output).
pub fn has_bodies(plan: &Plan) -> bool {
    !plan.campaign.npcs.content.npcs.is_empty() || !plan.campaign.quests.content.actors.is_empty()
}

/// Every body volume the delve ships, in a deterministic order: NPC anchors,
/// actor anchors, then every waypoint of every planned walk in plan order.
///
/// A body whose anchor does not resolve is skipped — `DW0325`/`DW0345`/`DW0360`
/// own dangling references, and reporting a geometry defect for one would send
/// the author to the wrong line.
fn volumes(plan: &Plan, moves: &[MovePlan], actor_moves: &[ActorMovePlan]) -> Vec<Volume> {
    let c = plan.campaign;
    let mut out = Vec::new();
    for (i, n) in c.npcs.content.npcs.iter().enumerate() {
        let Some(pos) = plan
            .point(n.area.as_str(), n.anchor.as_str())
            .or_else(|| plan.point_any(n.anchor.as_str()))
        else {
            continue;
        };
        out.push(Volume {
            id: n.id.as_str().to_string(),
            entity: crate::nav::npc_body_entity(n),
            pos: cell_feet(pos),
            path: format!("/content/npcs/{i}"),
            stage: "npcs",
            at: Where::Anchor {
                kind: "npc",
                anchor: n.anchor.as_str().to_string(),
            },
        });
    }
    for (i, a) in c.quests.content.actors.iter().enumerate() {
        let Some(pos) = plan.point_any(a.anchor.as_str()) else {
            continue;
        };
        out.push(Volume {
            id: a.id.as_str().to_string(),
            entity: crate::nav::actor_body_entity(a),
            pos: cell_feet(pos),
            path: format!("/content/actors/{i}"),
            stage: "quests",
            at: Where::Anchor {
                kind: "actor",
                anchor: a.anchor.as_str().to_string(),
            },
        });
    }
    for m in moves {
        let Some((entity, path)) = npc_body(plan, &m.npc) else {
            continue;
        };
        for (t, wp) in m.waypoints.iter().enumerate() {
            out.push(Volume {
                id: m.npc.clone(),
                entity: entity.clone(),
                pos: *wp,
                path: path.clone(),
                stage: "npcs",
                at: Where::Leg {
                    verb: "move-npc",
                    to_anchor: m.to_anchor.clone(),
                    tick: t,
                    ticks: m.ticks(),
                },
            });
        }
    }
    for m in actor_moves {
        let Some((entity, path)) = actor_body(plan, &m.actor) else {
            continue;
        };
        for (t, wp) in m.waypoints.iter().enumerate() {
            out.push(Volume {
                id: m.actor.clone(),
                entity: entity.clone(),
                pos: *wp,
                path: path.clone(),
                stage: "quests",
                at: Where::Leg {
                    verb: "move-actor",
                    to_anchor: m.to_anchor.clone(),
                    tick: t,
                    ticks: m.ticks(),
                },
            });
        }
    }
    out
}

/// The world position an entity summoned on cell `c` occupies: the cell's
/// horizontal centre, feet at the cell floor — exactly `emit`'s `ent_xyz`.
fn cell_feet(c: [i32; 3]) -> [f64; 3] {
    [c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 0.5]
}

/// `(effective entity, JSON pointer)` for a walking NPC.
fn npc_body(plan: &Plan, npc_id: &str) -> Option<(String, String)> {
    plan.campaign
        .npcs
        .content
        .npcs
        .iter()
        .enumerate()
        .find(|(_, n)| n.id.as_str() == npc_id)
        .map(|(i, n)| (crate::nav::npc_body_entity(n), format!("/content/npcs/{i}")))
}

/// `(effective entity, JSON pointer)` for a walking actor.
fn actor_body(plan: &Plan, actor_id: &str) -> Option<(String, String)> {
    plan.campaign
        .quests
        .content
        .actors
        .iter()
        .enumerate()
        .find(|(_, a)| a.id.as_str() == actor_id)
        .map(|(i, a)| {
            (
                crate::nav::actor_body_entity(a),
                format!("/content/actors/{i}"),
            )
        })
}

/// Prove no body — standing or walking — occupies the same space as block
/// geometry (`DW0450`, `DW0451`).
///
/// Returns the advisories on success; the presence of ANY error-tier violation
/// fails the build, with every error-tier violation named in one message so a
/// single build tells the author the whole list.
///
/// A walked leg reports at most its FIRST offending tick: a body dragged through
/// twenty blocks of rock is one defect, not two hundred, and the tick index plus
/// the cell is what locates it.
pub fn check_body_clearance(
    plan: &Plan,
    world: &World,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
) -> Result<Vec<Diagnostic>, Failure> {
    let mut errors: Vec<(Volume, [i32; 3], f64, f64)> = Vec::new();
    let mut warnings: Vec<Diagnostic> = Vec::new();
    // A leg reports once: `(id, to_anchor)` of legs that already have a finding.
    let mut reported_legs: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    for v in volumes(plan, moves, actor_moves) {
        let (w, h) = entity_dims(&v.entity);
        let Some((cell, hit)) = verdict(world, &v, w, h) else {
            continue;
        };
        if let Where::Leg { to_anchor, .. } = &v.at
            && !reported_legs.insert((v.id.clone(), to_anchor.clone()))
        {
            continue;
        }
        match hit {
            Hit::Solid => errors.push((v, cell, w, h)),
            Hit::Barrier | Hit::ModelOverhang => warnings.push(advisory(&v, cell, w, h, hit)),
        }
    }
    if let Some((first, cell, w, h)) = errors.first() {
        return Err(Failure {
            code: DW_BODY_CLEARANCE,
            message: clearance_error(first, *cell, *w, *h, &errors[1..]),
        });
    }
    Ok(warnings)
}

/// How a body is described in a message: ``actor `actor/polyphemus-walker`
/// (minecraft:warden, 0.9 × 2.9 blocks)``.
fn describe(v: &Volume, w: f64, h: f64) -> String {
    let kind = match &v.at {
        Where::Anchor { kind, .. } => *kind,
        Where::Leg { verb, .. } => match *verb {
            "move-npc" => "npc",
            _ => "actor",
        },
    };
    format!("{kind} `{}` ({}, {w} × {h} blocks)", v.id, v.entity)
}

/// Where the body is, in words: ``at its spawn anchor `anchor/mouth-side`
/// [6, 69, -45]`` or ``at tick 42 of 289 of its `move-actor` walk to …``.
fn locate(v: &Volume) -> String {
    match &v.at {
        Where::Anchor { anchor, .. } => format!(
            "at its anchor `{anchor}` {:?}, where it is summoned at {}",
            feet_cell(v.pos),
            fmt_pos(v.pos)
        ),
        Where::Leg {
            verb,
            to_anchor,
            tick,
            ticks,
        } => format!(
            "at tick {tick} of {ticks} of its `{verb}` walk to `{to_anchor}`, position {}",
            fmt_pos(v.pos)
        ),
    }
}

/// The feet cell a body position sits in.
fn feet_cell(p: [f64; 3]) -> [i32; 3] {
    [
        p[0].floor() as i32,
        p[1].floor() as i32,
        p[2].floor() as i32,
    ]
}

/// A body position as the emitted command spells it.
fn fmt_pos(p: [f64; 3]) -> String {
    format!("{} {} {}", p[0], p[1], p[2])
}

/// The `DW0450` error: the body is inside the rock.
fn clearance_error(
    v: &Volume,
    cell: [i32; 3],
    w: f64,
    h: f64,
    rest: &[(Volume, [i32; 3], f64, f64)],
) -> String {
    let needs = h.ceil() as i32;
    let mut msg = format!(
        "{} is INSIDE SOLID BLOCK GEOMETRY {}: its hitbox overlaps the solid block at {cell:?}. \
         The body needs a clear volume {w} blocks across and {h} blocks tall — {needs} cells of \
         headroom in the column it stands in — and the assembled world does not give it one \
         there, so the entity ships embedded in the wall (a `NoGravity` puppet never settles out \
         of it) or is ejected somewhere the campaign never staged. \
         A walked destination cannot land like this — `move-npc`/`move-actor` snap their \
         endpoints to a standable cell and A* only steps through passable cells — but a `summon` \
         does no snapping, so an anchor is exactly where the body appears. \
         Prescription: move the anchor to a cell with real clearance, or give the walk a corridor \
         the body fits. Do NOT make the body smaller to fit: the dims table states what the mob \
         IS, and `move-npc` deliberately plans on the player footprint, so a big-bodied NPC walked \
         down a player-sized corridor is a route that was never sized for it.",
        describe(v, w, h),
        locate(v),
    );
    if !rest.is_empty() {
        let _ = write!(
            msg,
            " {} further body-clearance violation(s) in this build:",
            rest.len()
        );
        for (o, c, ow, oh) in rest {
            let _ = write!(
                msg,
                " {} {} → solid at {c:?};",
                describe(o, *ow, *oh),
                locate(o)
            );
        }
    }
    msg
}

/// The `DW0451` advisory: the hitbox is clear, but the body will still read as
/// clipping.
fn advisory(v: &Volume, cell: [i32; 3], w: f64, h: f64, hit: Hit) -> Diagnostic {
    let body = describe(v, w, h);
    let at = locate(v);
    let text = match hit {
        Hit::ModelOverhang => format!(
            "{body} is {at} with its hitbox clear of block geometry, but a solid block at {cell:?} \
             lies within {MODEL_MARGIN} blocks of it horizontally. Vanilla mob models render past \
             their collision box — a warden's arms and shoulders, an iron golem's, a ravager's \
             horns — so a body flush against a wall LOOKS embedded even though nothing physically \
             overlaps. Advisory, not an error: the exact overhang of a given model is client \
             render geometry the compiler has no data for, so this is a measurement against a \
             named {MODEL_MARGIN}-block margin, never a verdict. Prescription: give the body a \
             cell of clearance from the wall, or confirm the framing in playtest."
        ),
        _ => format!(
            "{body} is {at} and its volume contains the 1.5-tall barrier (fence / wall / closed \
             fence gate) at {cell:?}. Advisory, not an error: the occupancy model fills such a \
             cell for pathing, but the real block is a narrow post or panel, so whether the body \
             interpenetrates it depends on sub-block shape the compiler does not carry. \
             Prescription: check the cell — a body standing in a fence line is usually a placement \
             mistake, a body beside one is usually fine."
        ),
    };
    Diagnostic::warning(DW_BODY_CLEARANCE_ADVISORY, v.stage, v.path.clone(), text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// A body volume at a cell, with the island's giant's dims.
    fn giant_at(cell: [i32; 3]) -> Volume {
        Volume {
            id: "actor/polyphemus-walker".to_string(),
            entity: "minecraft:warden".to_string(),
            pos: cell_feet(cell),
            path: "/content/actors/0".to_string(),
            stage: "quests",
            at: Where::Anchor {
                kind: "actor",
                anchor: "anchor/mouth-side".to_string(),
            },
        }
    }

    /// An [`crate::assembled::Occupancy`] with only its `solid` set filled.
    fn occ(solid: BTreeSet<[i32; 3]>) -> crate::assembled::Occupancy {
        crate::assembled::Occupancy {
            solid,
            tall: BTreeSet::new(),
            use_gates: BTreeSet::new(),
            flooded: BTreeSet::new(),
            partial: std::collections::BTreeMap::new(),
        }
    }

    /// A floor at `y-1` over `x,z in 0..8`, with a 3-high open room above it.
    fn room(y: i32) -> BTreeSet<[i32; 3]> {
        let mut solid = BTreeSet::new();
        for x in 0..8 {
            for z in 0..8 {
                solid.insert([x, y - 1, z]);
                solid.insert([x, y + 3, z]);
            }
        }
        solid
    }

    /// The island's exact defect: the anchor cell and the two above it are rock.
    #[test]
    fn a_warden_summoned_into_rock_is_an_error() {
        let mut solid = room(69);
        for dy in 0..3 {
            solid.insert([6, 69 + dy, 5]);
        }
        let world = World::from_solid_cells(solid);
        let v = giant_at([6, 69, 5]);
        let (w, h) = entity_dims(&v.entity);
        assert!(
            matches!(verdict(&world, &v, w, h), Some((_, Hit::Solid))),
            "a body summoned inside a solid column must be the error tier"
        );
    }

    /// The same giant in the open room next door is silent at both tiers.
    #[test]
    fn a_warden_with_three_cells_of_headroom_is_clean() {
        let world = World::from_solid_cells(room(69));
        let v = giant_at([3, 69, 3]);
        let (w, h) = entity_dims(&v.entity);
        assert!(verdict(&world, &v, w, h).is_none());
    }

    /// 2.9 blocks tall needs THREE cells: a body in a player-sized 2-high room
    /// has its head in the ceiling. The mechanism behind a `move-npc` leg that
    /// routes a big body down a player-sized corridor.
    #[test]
    fn a_two_high_corridor_is_an_error_for_a_warden() {
        let mut solid = BTreeSet::new();
        for x in 0..8 {
            for z in 0..8 {
                solid.insert([x, 68, z]); // floor
                solid.insert([x, 71, z]); // ceiling: only y=69,70 are clear
            }
        }
        let world = World::from_solid_cells(solid);
        let v = giant_at([3, 69, 3]);
        let (w, h) = entity_dims(&v.entity);
        let (cell, hit) = verdict(&world, &v, w, h).expect("2.9 tall does not fit 2 cells");
        assert_eq!(hit, Hit::Solid);
        assert_eq!(cell, [3, 71, 3]);
    }

    /// …and the same corridor is fine for a 1.8-tall player-model mannequin, so
    /// the rule is about the real body, not about being strict.
    #[test]
    fn a_two_high_corridor_fits_a_mannequin() {
        let mut solid = BTreeSet::new();
        for x in 0..8 {
            for z in 0..8 {
                solid.insert([x, 68, z]);
                solid.insert([x, 71, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let mut v = giant_at([3, 69, 3]);
        v.entity = "minecraft:mannequin".to_string();
        let (w, h) = entity_dims(&v.entity);
        assert!(verdict(&world, &v, w, h).is_none());
    }

    /// A bottom slab is half a block, so a body standing on the cell above it is
    /// clear even though the slab's cell is `solid` — the proof reads collision
    /// volumes, not cells.
    #[test]
    fn a_bottom_slab_below_the_feet_is_not_an_obstruction() {
        let mut solid = BTreeSet::new();
        for x in 0..8 {
            for z in 0..8 {
                solid.insert([x, 68, z]);
                solid.insert([x, 72, z]);
            }
        }
        let mut o = occ(solid);
        // A slab sitting in the feet cell's own floor cell: collision top 8/16.
        o.partial.insert([3, 68, 3], 8);
        let world = World::from_occupancy(o, crate::nav::Premises::geometry_only());
        let v = giant_at([3, 69, 3]);
        let (w, h) = entity_dims(&v.entity);
        assert!(verdict(&world, &v, w, h).is_none());
    }

    /// The 0.9-wide warden leaves 0.05 of its cell empty on each side, so a wall
    /// in the neighbouring cell does not touch the hitbox — but the model does
    /// overhang into it, which is the advisory tier, not a build failure.
    #[test]
    fn a_wall_one_cell_over_is_the_model_overhang_warning() {
        let mut solid = room(69);
        for dy in 0..3 {
            solid.insert([4, 69 + dy, 3]);
        }
        let world = World::from_solid_cells(solid);
        let v = giant_at([3, 69, 3]);
        let (w, h) = entity_dims(&v.entity);
        let (cell, hit) = verdict(&world, &v, w, h).expect("a flush wall must be measured");
        assert_eq!(hit, Hit::ModelOverhang);
        assert_eq!(cell, [4, 69, 3]);
        let d = advisory(&v, cell, w, h, hit);
        assert_eq!(d.code, DW_BODY_CLEARANCE_ADVISORY);
        assert_eq!(d.severity, delvewright_dsl::Severity::Warning);
    }

    /// The discrimination the margin exists for: a 0.6-wide player-model body
    /// keeps 0.2 blocks of its own cell on each side, so a wall in the next cell
    /// does not even warn. An NPC standing against a wall is the most ordinary
    /// staging in the DSL and must not produce a diagnostic.
    #[test]
    fn a_player_model_body_flush_against_a_wall_is_silent() {
        let mut solid = room(69);
        for dy in 0..3 {
            solid.insert([4, 69 + dy, 3]);
        }
        let world = World::from_solid_cells(solid);
        let mut v = giant_at([3, 69, 3]);
        v.entity = "minecraft:mannequin".to_string();
        let (w, h) = entity_dims(&v.entity);
        assert_eq!(w, 0.6);
        assert!(verdict(&world, &v, w, h).is_none());
    }

    /// …and the same flush warden **mid-walk** is silent: a walker hugging a
    /// corridor wall is the corridor's dimensions, not a defect. Only the hard
    /// hitbox rule follows a body through its legs.
    #[test]
    fn a_walked_waypoint_flush_against_a_wall_is_silent() {
        let mut solid = room(69);
        for dy in 0..3 {
            solid.insert([4, 69 + dy, 3]);
        }
        let world = World::from_solid_cells(solid);
        let mut v = giant_at([3, 69, 3]);
        v.at = Where::Leg {
            verb: "move-actor",
            to_anchor: "anchor/mouth".to_string(),
            tick: 7,
            ticks: 40,
        };
        let (w, h) = entity_dims(&v.entity);
        assert!(verdict(&world, &v, w, h).is_none());
        // …but rock in the body's own column still stops the build mid-leg.
        let mut walled = room(69);
        for dy in 0..3 {
            walled.insert([3, 69 + dy, 3]);
        }
        let walled = World::from_solid_cells(walled);
        assert!(matches!(verdict(&walled, &v, w, h), Some((_, Hit::Solid))));
    }

    /// Two cells of clearance is silent at both tiers: the margin is 0.2, not a
    /// licence to complain about every wall in the room.
    #[test]
    fn two_cells_of_clearance_is_silent() {
        let mut solid = room(69);
        for dy in 0..3 {
            solid.insert([5, 69 + dy, 3]);
        }
        let world = World::from_solid_cells(solid);
        let v = giant_at([3, 69, 3]);
        let (w, h) = entity_dims(&v.entity);
        assert!(verdict(&world, &v, w, h).is_none());
    }

    /// A fence in the body's own cell is the advisory tier, never the error
    /// tier: the occupancy model fills the cell, the real block is a post.
    #[test]
    fn a_fence_inside_the_body_is_advisory() {
        let mut solid = BTreeSet::new();
        for x in 0..8 {
            for z in 0..8 {
                solid.insert([x, 68, z]);
                solid.insert([x, 72, z]);
            }
        }
        let mut o = occ(solid);
        o.tall.insert([3, 69, 3]);
        let world = World::from_occupancy(o, crate::nav::Premises::geometry_only());
        let v = giant_at([3, 69, 3]);
        let (w, h) = entity_dims(&v.entity);
        let (cell, hit) = verdict(&world, &v, w, h).expect("a fence in the body must be measured");
        assert_eq!(hit, Hit::Barrier);
        assert_eq!(cell, [3, 69, 3]);
    }

    /// The codes are the ones the reference documents.
    #[test]
    fn codes_are_dw0450_and_dw0451() {
        assert_eq!(DW_BODY_CLEARANCE, "DW0450");
        assert_eq!(DW_BODY_CLEARANCE_ADVISORY, "DW0451");
    }
}
