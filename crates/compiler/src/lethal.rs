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
use delvewright_dsl::DwCode;

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
pub const DW_LETHAL_RESPAWN_SEAT: DwCode = DwCode::every_version("DW0511");

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

/// Every place the campaign PUTS something: the party's respawn seats, then every
/// declared body's post. Each carries the label a diagnostic names it by.
///
/// Deterministic throughout — entry, then checkpoints in content order, then NPCs
/// in declaration order with their cast placements, then actors (ADR-0006).
fn posted_places(plan: &Plan, entry: Option<[i32; 3]>) -> Vec<(String, [i32; 3])> {
    let mut out = Vec::new();
    if let Some(pos) = entry {
        out.push(("the campaign's entry spawn".to_string(), pos));
    }
    for cp in &plan.checkpoints {
        let kind = if cp.rest { "bonfire" } else { "checkpoint" };
        out.push((format!("{kind} anchor `{}`", cp.anchor), cp.pos));
    }
    let c = plan.campaign;
    for npc in &c.npcs.content.npcs {
        if let Some(pos) = plan.point_any(npc.anchor.as_str()) {
            out.push((format!("npc `{}`'s post `{}`", npc.id, npc.anchor), pos));
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
                out.push((
                    format!("npc `{npc}`'s `cast` placement `{at}` in quest `{}`", q.id),
                    pos,
                ));
            }
        }
    }
    for a in &c.quests.content.actors {
        if let Some(pos) = plan.point_any(a.anchor.as_str()) {
            out.push((format!("actor `{}`'s post `{}`", a.id, a.anchor), pos));
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
pub fn check_respawn_seats(plan: &Plan, entry: Option<[i32; 3]>) -> Result<usize, Failure> {
    let seats = posted_places(plan, entry);
    if plan.lethal_volumes.is_empty() {
        return Ok(seats.len());
    }
    for (label, pos) in &seats {
        let blamed: Vec<&str> = plan
            .lethal_volumes
            .iter()
            .filter(|v| v.contains(*pos))
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
        return Err(Failure {
            code: DW_LETHAL_RESPAWN_SEAT,
            message: format!(
                "{label} at {pos:?} lies INSIDE lethal volume(s) {names}. Whatever the campaign \
                 posts here is put here by declaration, not by walking, so no route proof can \
                 see it: a respawn seat means the party dies on arrival and is re-seated to die \
                 again on every death, forever (`/spawnpoint` is only a hint and the engine \
                 re-seats on the death edge, so nothing downstream can rescue it); a posted body \
                 means the volume deletes it on the first tick and the delve loses it in \
                 silence. Move the post out of the volume, or shrink the volume's `extent` so it \
                 does not cover it; do NOT delete the volume to silence the proof."
            ),
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
