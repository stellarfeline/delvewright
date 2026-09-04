//! Lethal volumes (DSL v0.10, spec-0031): the proofs a box that kills owes the
//! completability model, and the binding ledger that says what they looked at.
//!
//! ## Where the reasoning lives, and why it is split
//!
//! A lethal volume is geometry, so most of its completability reasoning is not
//! here: [`crate::nav::World`] carries its cells as **impassable** and every route
//! proof in the engine inherits that for free — the critical path (`DW0510`), the
//! checkpoint no-stranding proof (`DW0315`), the branch paths, the trap forced-cell
//! set, the exported harness waypoints. That is the whole point of putting it in
//! the world rather than in a check of its own: a fourth consumer inherits the
//! proof instead of re-deriving it, exactly as `close-gate`'s seal does.
//!
//! What is left is the one obligation routing cannot see, because it is not about
//! walking: **the places the campaign PUTS something.** A respawn seat or a posted
//! body inside a lethal volume routes perfectly and is killed on arrival — the
//! party forever (the death loop), an NPC once and silently. Both are
//! [`DW_LETHAL_RESPAWN_SEAT`], because both are the same defect: a position
//! reached by declaration rather than by walking.
//!
//! `seats` in the ledger counts every such place, not only the respawn ones.
//!
//! ## A body has a width
//!
//! "Inside a volume" is a question about a **body**, not about a cell. The volume
//! kills with a box selector, and the server adjudicates a box selector on hitbox
//! intersection — so a 1.4-wide spider standing on the cell beside a volume's
//! face is inside it, and a 0.6-wide villager on the same cell is not. Every
//! place here therefore carries the hitbox of the body that lands on it
//! ([`PostedPlace`]) and is judged by
//! [`delvewright_dsl::metrics::body_meets_volume`], the one answer the routing
//! model and the emitted selector are also written against.
//!
//! One class of place is chosen by the compiler rather than by the campaign — a
//! **wave's seats**, taken from whatever standable footing the anchor's room
//! offers. That footing is proven for a player's body, so the seats that survive
//! it are safe for a walker and not necessarily for a wider mob; and the author
//! has no post to move. It is therefore its own diagnostic,
//! [`DW_LETHAL_WAVE_SEAT`], with its own prescription — see [`ChosenBy`].
//!
//! ## Binding (`docs/reference/playtest-methodology.md` rule 1)
//!
//! [`LethalGate`] states what was examined: how many volumes were declared, how
//! many resolved, how many cells they hold, how many respawn seats were tested and
//! how many critical-path legs were routed against them. A zero volume count is
//! reported as unbound rather than as a pass — a campaign that declares no volume
//! emits no ledger at all, so a ledger that exists and says zero is a finding.

use delvewright_dsl::Campaign;

use crate::failure::Failure;
use crate::plan::{LethalVolumePlan, Plan};
use delvewright_dsl::{DwCode, ExitTier};

/// `DW0511`: a **posted place** — somewhere the campaign requires the party or a
/// declared body to BE — lies inside a lethal volume (spec-0031).
///
/// One rule, because it is one defect: *a body is put here by declaration, not by
/// walking, so no route proof can see it.* Two families of site fall under it.
///
/// * **Respawn seats** — the campaign's entry spawn, a `set-checkpoint` cell, a
///   `bonfire` cell. The death loop: the party dies on arrival and is re-seated to
///   die again, forever. `/spawnpoint` is only a hint and the engine re-seats on
///   the death edge, so nothing downstream can rescue it. The exact dual of
///   `DW0315`/`DW0316` for the hazard the party respawns *into*.
/// * **Posted bodies** — a stage-2 NPC's anchor, a per-quest `cast` placement, a
///   stage-5 actor's anchor. A volume's entity sweep exempts the engine's own
///   machinery types and deliberately NOT content bodies (a mob that walks into
///   the lava dies, which is the mechanism working) — so an NPC posted inside one
///   is deleted on the first tick, the delve loses its speaker, and every static
///   proof stays green. Found while writing this feature's own CI fixture, which
///   is exactly the shape the rule now refuses.
pub const DW_LETHAL_RESPAWN_SEAT: DwCode = DwCode::every_version("DW0511", ExitTier::Build);

/// `DW0879`: a **wave seat** — a cell the COMPILER chose to stand one of a wave's
/// mobs on — is a cell that body's own hitbox meets a lethal volume from.
///
/// A separate code from [`DW_LETHAL_RESPAWN_SEAT`] because the remedy is
/// separate, and the remedy is the whole content of a diagnostic. Every place
/// `DW0511` names is a cell the author WROTE: an anchor, a checkpoint, a `cast`
/// placement — so "move the post out of the volume" is an instruction they can
/// carry out. A wave seat is chosen by `emit::plan_wave_spawns` from whatever
/// standable footing the anchor's own room offers; there is no post to move, and
/// an author told to move one would go looking for a declaration that does not
/// exist. What they can move is the wave's `anchor`, the volume's `extent`, or
/// the mob count that forces the seating to spread as far as the volume.
///
/// It is also the case that separates a body's WIDTH from its cell, which is the
/// second reason it cannot share `DW0511`'s wording: the routing model already
/// keeps a **player** out of the ring around a volume, so the seats a wave can be
/// given are all safe for a 0.6-wide body. What is left over is exactly the
/// bodies that are wider than the walker the footing was proven for — a 1.4-wide
/// spider on a cell a player stands in perfectly safely.
///
/// `every_version` for `DW0511`'s reason: a campaign below the `dsl_version` that
/// introduced `lethal_volumes[]` cannot declare one, so the rule reaches nothing
/// there and there is no obligation for a fence to grandfather.
pub const DW_LETHAL_WAVE_SEAT: DwCode = DwCode::every_version("DW0879", ExitTier::Build);

/// The binding ledger for the lethal-volume proofs.
#[derive(Clone, Debug, Default)]
pub struct LethalGate {
    /// Volumes the campaign declared.
    pub declared: usize,
    /// Volumes that resolved to a box on the solved layout. A gap between this
    /// and [`Self::declared`] means an anchor no placed piece provides — already
    /// `DW0142` at validation, restated here so a reader of the ledger alone
    /// cannot mistake a dropped volume for a proven one.
    pub resolved: usize,
    /// World cells those boxes cover — what the navigation model actually made
    /// impassable.
    pub cells: usize,
    /// Respawn seats tested against every volume (`DW0511`).
    pub seats: usize,
    /// Critical-path legs routed over the world with lethality applied
    /// (`DW0510` / `DW0311`).
    pub legs: usize,
    /// PackTest templates generated for these volumes — the runtime half. A
    /// compile-time-only green over a runtime mechanism is the vacuity this
    /// number exists to make visible.
    pub packtests: usize,
}

impl LethalGate {
    /// Whether this proof matched nothing at all.
    pub fn unbound(&self) -> bool {
        self.resolved == 0
    }

    /// The ledger as the `validation/lethal-gate.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "volumes": { "declared": self.declared, "resolved": self.resolved },
            "cells": self.cells,
            "respawn_seats_examined": self.seats,
            "critical_path_legs_examined": self.legs,
            "packtest_templates": self.packtests,
            "unbound": self.unbound(),
        })
    }
}

/// One posted place: what a diagnostic calls it, the cell the campaign puts a
/// body on, and **the body that lands there**.
///
/// The third field is the whole of what a cell could not say. A volume kills by a
/// box selector and vanilla adjudicates that on hitbox intersection, so whether a
/// post is inside a volume is a question about a body's box and not about a cell:
/// a 1.4-wide iron golem seated on the cell beside a pit's face is standing in
/// the pit as far as the selector is concerned, and a 0.6-wide villager on the
/// same cell is not.
pub struct PostedPlace {
    /// What the diagnostic names this place.
    pub label: String,
    /// The cell the campaign posts a body on.
    pub cell: [i32; 3],
    /// The hitbox of the body that lands there.
    pub body: delvewright_dsl::metrics::Body,
    /// Who chose the cell — which decides the remedy, and so the code.
    pub chosen_by: ChosenBy,
}

/// **Who chose a posted place's cell.** The two answers get different
/// diagnostics, because the author can only act on the one they wrote.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChosenBy {
    /// The campaign: an entry spawn, a checkpoint anchor, an NPC's or actor's
    /// post, a `cast` placement. [`DW_LETHAL_RESPAWN_SEAT`], whose prescription
    /// is *move the post, or shrink the volume* — both things the author wrote.
    Campaign,
    /// The **compiler**, seating a wave's mobs on the standable footing its room
    /// offers. [`DW_LETHAL_WAVE_SEAT`]: there is no post to move, so the
    /// prescription is about the room, the anchor and the volume instead.
    WaveSeating,
}

/// The body a stage-2 NPC actually wears, as a hitbox — through
/// [`crate::nav::npc_body_entity`], so a skinned NPC is measured as the mannequin
/// it ships as rather than as the `base_entity` it declares.
fn npc_body(n: &delvewright_dsl::Npc) -> delvewright_dsl::metrics::Body {
    let (w, h) = crate::nav::entity_dims(&crate::nav::npc_body_entity(n));
    delvewright_dsl::metrics::Body::new(w, h)
}

/// Every place the campaign PUTS something: the party's respawn seats, then every
/// declared body's post, then every cell a wave's mobs are seated on.
///
/// Deterministic throughout — entry, then checkpoints in content order, then NPCs
/// in declaration order with their cast placements, then actors, then waves in
/// declaration order with their stacks in declaration order (ADR-0006).
///
/// `wave_seats` is the seating [`crate::plan::wave_seats`] pairs off
/// `emit::plan_wave_spawns`'s answer. It is an argument rather than something
/// derived here because a wave's cells are a **measurement of the assembled
/// world** — the standable footing its room actually offers — and the plan alone
/// cannot state them. A campaign whose waves have not been seated (a proof run
/// before that pass) passes an empty map and the wave arm binds to nothing, which
/// is why the caller's ledger counts what it examined.
fn posted_places(
    plan: &Plan,
    entry: Option<[i32; 3]>,
    wave_seats: &std::collections::BTreeMap<String, Vec<[i32; 3]>>,
) -> Vec<PostedPlace> {
    let player = delvewright_dsl::metrics::Body::PLAYER;
    let mut out: Vec<PostedPlace> = Vec::new();
    fn declared(
        out: &mut Vec<PostedPlace>,
        label: String,
        cell: [i32; 3],
        body: delvewright_dsl::metrics::Body,
    ) {
        out.push(PostedPlace {
            label,
            cell,
            body,
            chosen_by: ChosenBy::Campaign,
        });
    }
    let push = declared;
    if let Some(pos) = entry {
        push(
            &mut out,
            "the campaign's entry spawn".to_string(),
            pos,
            player,
        );
    }
    for cp in &plan.checkpoints {
        let kind = if cp.rest { "bonfire" } else { "checkpoint" };
        push(
            &mut out,
            format!("{kind} anchor `{}`", cp.anchor),
            cp.pos,
            player,
        );
    }
    let c = plan.campaign;
    for npc in &c.npcs.content.npcs {
        if let Some(pos) = plan.point_any(npc.anchor.as_str()) {
            push(
                &mut out,
                format!("npc `{}`'s post `{}`", npc.id, npc.anchor),
                pos,
                npc_body(npc),
            );
        }
    }
    // Per-quest `cast` placements move an NPC between beats, so the stage-2 anchor
    // alone is not the whole answer: a keeper who is safe at his post and posted
    // into the pit for act two is the same defect one quest later.
    for q in &c.quests.content.quests {
        for (npc, entry) in &q.cast {
            for pl in entry.placements() {
                let Some(at) = pl.at.anchor() else { continue };
                let Some(pos) = plan.point_any(at.as_str()) else {
                    continue;
                };
                let body = c
                    .npcs
                    .content
                    .npcs
                    .iter()
                    .find(|n| n.id.as_str() == npc.as_str())
                    .map_or(player, npc_body);
                push(
                    &mut out,
                    format!("npc `{npc}`'s `cast` placement `{at}` in quest `{}`", q.id),
                    pos,
                    body,
                );
            }
        }
    }
    for a in &c.quests.content.actors {
        if let Some(pos) = plan.point_any(a.anchor.as_str()) {
            let (w, h) = crate::nav::entity_dims(&crate::nav::actor_body_entity(a));
            push(
                &mut out,
                format!("actor `{}`'s post `{}`", a.id, a.anchor),
                pos,
                delvewright_dsl::metrics::Body::new(w, h),
            );
        }
    }
    for w in &c.quests.content.waves {
        let Some(cells) = wave_seats.get(w.id.as_str()) else {
            continue;
        };
        for (entity, cell) in crate::plan::wave_seats(w, cells) {
            let (bw, bh) = crate::nav::entity_dims(entity);
            out.push(PostedPlace {
                label: format!("wave `{}`'s seat for `{entity}`", w.id),
                cell,
                body: delvewright_dsl::metrics::Body::new(bw, bh),
                chosen_by: ChosenBy::WaveSeating,
            });
        }
    }
    out
}

/// Prove no respawn seat sits inside a lethal volume (`DW0511`).
///
/// `entry` is the campaign's resolved entry-spawn cell, threaded in by the caller
/// (the emitter already resolves it); `None` for a layout with no entry anchor,
/// which is `DW0345` upstream and not this proof's finding to make.
///
/// Returns the seats examined on success, so the caller's ledger reports a real
/// count rather than a number derived a second time.
pub fn check_respawn_seats(
    plan: &Plan,
    entry: Option<[i32; 3]>,
    wave_seats: &std::collections::BTreeMap<String, Vec<[i32; 3]>>,
) -> Result<usize, Failure> {
    let seats = posted_places(plan, entry, wave_seats);
    if plan.lethal_volumes.is_empty() {
        return Ok(seats.len());
    }
    for place in &seats {
        // The body's own box, at the coordinates a summon writes for that cell —
        // `nav::cell_center` is the one place that says where a body seated on a
        // cell stands, and `metrics::body_meets_volume` the one place that says
        // whether a box meets a volume.
        let feet = crate::nav::cell_center(place.cell);
        let blamed: Vec<&str> = plan
            .lethal_volumes
            .iter()
            .filter(|v| {
                delvewright_dsl::metrics::body_meets_volume(
                    feet, place.body, v.region.0, v.region.1,
                )
            })
            .map(|v| v.id.as_str())
            .collect();
        if blamed.is_empty() {
            continue;
        }
        let names = blamed
            .iter()
            .map(|i| format!("`{i}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let label = &place.label;
        let pos = place.cell;
        let (w, h) = (place.body.width, place.body.height);
        return Err(match place.chosen_by {
            ChosenBy::Campaign => Failure {
                code: DW_LETHAL_RESPAWN_SEAT,
                message: format!(
                    "{label} at {pos:?} puts a body whose hitbox is {w} x {h} blocks INSIDE \
                     lethal volume(s) {names}. A volume kills by a box selector and the server \
                     adjudicates that on hitbox INTERSECTION, so a body wider than nothing \
                     reaches out of the cell it stands on: this post is inside the volume even \
                     where its cell is not. Whatever the campaign puts here is put here by \
                     declaration, not by walking, so no route proof can see it: a respawn seat \
                     means the party dies on arrival and is re-seated to die again on every \
                     death, forever (`/spawnpoint` is only a hint and the engine re-seats on the \
                     death edge, so nothing downstream can rescue it); a posted body means the \
                     volume deletes it on the first tick and the delve loses it in silence. Move \
                     the post clear of the volume — clear of its FACES, not merely out of its \
                     cells — or shrink the volume's `extent`; do NOT delete the volume to \
                     silence the proof."
                ),
            },
            ChosenBy::WaveSeating => Failure {
                code: DW_LETHAL_WAVE_SEAT,
                message: format!(
                    "{label} is the cell {pos:?}, and a body whose hitbox is {w} x {h} blocks \
                     standing there is INSIDE lethal volume(s) {names}: a volume kills by a box \
                     selector and the server adjudicates that on hitbox INTERSECTION, so a body \
                     this wide reaches out of its own cell and into the volume's face. The mob \
                     is killed on the tick it is summoned, the wave's counter never comes down, \
                     and any objective that waits for the wave to be cleared waits forever. \
                     Nothing here was authored as a position: the compiler seats a wave on the \
                     standable footing its anchor's room offers, and that footing is proven for \
                     a PLAYER's body, which is narrower than this one. So the fix is not a post \
                     to move. Move the wave's `anchor` further from the volume, shrink the \
                     volume's `extent`, or reduce the wave's mob count so its seating does not \
                     have to spread as far as the volume's edge. Do NOT delete the volume, and \
                     do NOT widen the room's footing to push the seating outward — that only \
                     moves which mob lands on the edge."
                ),
            },
        });
    }
    Ok(seats.len())
}

/// The ledger for a finished build: what the campaign declared, what resolved,
/// and what each proof examined.
pub fn gate(
    c: &Campaign,
    volumes: &[LethalVolumePlan],
    cells: usize,
    seats: usize,
    legs: usize,
    packtests: usize,
) -> LethalGate {
    LethalGate {
        declared: c.quests.content.lethal_volumes.len(),
        resolved: volumes.len(),
        cells,
        seats,
        legs,
        packtests,
    }
}
