//! The `teleport` verb's one compile-time obligation, and the ledger that says
//! what it looked at (DSL v0.10, spec-0031).
//!
//! ## Why there is no runtime exemption list
//!
//! `lethal_volumes[]` sweeps its box with `@e[…,type=!minecraft:interaction,…]`
//! and five machinery types by name ([`crate::emit`]'s `LETHAL_EXEMPT_TYPES`),
//! because a volume drawn across a cutscene dolly would otherwise erase the
//! camera. The obvious move is to copy that list here. **It is the wrong move,
//! and copying it would have been silently wrong in the one case the owner ruled
//! on by name.**
//!
//! A stage-2 NPC is *two* entities sharing one `dw_npc_<id>` tag: a body and a
//! co-located `minecraft:interaction` hitbox that carries its dialogue. Owner
//! ruling 2026-08-08 is that everyone on the car travels, players and entities
//! alike — a cargo lift is the same mechanism. Exempting `minecraft:interaction`
//! would teleport the speaker and leave the thing players click standing in the
//! old car, and nothing anywhere would notice: the NPC is present at the
//! destination and simply cannot be talked to.
//!
//! So the teleport's selector is **total over bodies** — `tp
//! @e[<box>,tag=!dw_fixture] <cell>`, no `type=`, no `limit=`, no `sort=` — and
//! the cases an exemption list would have hidden are refused at compile time
//! instead. That trade is available here and was not available to the lethal
//! volume, for a reason worth stating: a volume damages whatever *wanders* in,
//! which the compiler cannot enumerate, while a teleport's harm is to what the
//! **compiler itself placed**, which it can. A proof beats a list that grows with
//! the engine.
//!
//! ## The one narrowing, and why it is not an exemption list (`DW0544`)
//!
//! `tag=!dw_fixture` is not a roster of types wearing a tag's clothes. It is a
//! **class the object declares about itself** at the moment the engine summons
//! it: *my position IS engine state*. See [`crate::affordance`] for the class and
//! its proof; the short form is that types and classes answer different
//! questions, and only one of them is the question a moving verb has.
//!
//! An NPC's dialogue hitbox and a recovery stake's marker are the same vanilla
//! type — `minecraft:interaction` — and must be treated in opposite ways by a
//! teleport. The hitbox travels, or the delve keeps its speaker and loses its
//! speech. The marker stays, or the delve carries a fact away from the position
//! that recorded it: the stake's ledger holds the marker's coordinates, and
//! `stk_gc_<s>` retires a marker no player holds a wager *at that position*, so
//! the tick after the ride the marker is gone and the wager with it. A `type=`
//! term cannot tell the two apart. A class can, and it is decided once, at the
//! object, rather than once per verb.
//!
//! This is also why the stake could not simply enter [`DW_TELEPORT_BOUND_AFFORDANCE`]
//! below. That proof is compile-time geometry and a marker's position is chosen
//! at RUNTIME; the class needs no position at all, which is exactly the property
//! the runtime-placed half of the engine's furniture requires.
//!
//! **This is CLAUDE.md's "a capability belongs to the object class it acts on,
//! not to the verb that first needed it" paying off in the direction people
//! forget.** The rule is usually cited to stop a second verb re-implementing a
//! capability privately. Here it earns its keep the other way round: the
//! exemption list is keyed to what a *lethal volume* does to an entity — delete
//! it — and that keying is invisible in the list itself, which reads like a
//! neutral roster of "engine machinery". Inherited by a verb that *moves*
//! entities rather than deleting them, the same roster is not merely
//! unnecessary, it is **actively wrong**, and wrong in the silent direction: the
//! delve keeps its speaker, at the right coordinates, and simply cannot be
//! talked to. Nothing reds. Reusing a list because it names the right *types* —
//! without asking what the verb that wrote it was doing to them — is how a
//! second consumer inherits a first consumer's assumptions in the dark.
//!
//! ## What is refused: an affordance bound to hardware the teleport cannot move
//!
//! Every interaction affordance the engine summons is anchored to a compile-time
//! cell and, for most of them, to a *block* placed at that cell — a bonfire's
//! campfire, a shortcut lever, a sealed gate's blocks. Teleporting the entity
//! moves the half a player's crosshair reaches and leaves the half they can see,
//! so the affordance is still visible, still on the map, and answers nothing.
//! That is [`DW_TELEPORT_BOUND_AFFORDANCE`].
//!
//! The affordance set is **not enumerated here**. It is
//! [`crate::eclipse::affordances`], the same single authority `DW0359` measures
//! bodies against, plus the seal shells `close-gate` arms
//! ([`crate::plan::SealHintPlan::shell_cells`], which `DW0422` owns). A future
//! affordance therefore enters this proof by existing, not by someone
//! remembering to add it.
//!
//! Content bodies — NPCs, actor puppets, wave mobs — are deliberately NOT
//! refused: moving them is the mechanism working, and it is exactly what the
//! cargo-lift ruling asks for.
//!
//! Engine furniture with **no** compile-time cell — a recovery stake's marker,
//! a cutscene's return mark — is not refused here either, and cannot be: an
//! author cannot move a thing whose position the runtime chooses. It is excluded
//! from the selector instead (`DW0544`). The split is the whole design: **a place
//! the author can move is refused; a place only the runtime can put down is
//! skipped.**
//!
//! ## Binding (`docs/reference/playtest-methodology.md` rule 1)
//!
//! [`TeleportGate`] states what was examined: how many `teleport` effects were
//! declared, how many resolved to a box, how many cells those boxes cover, and
//! how many engine affordances were tested against them. A campaign that
//! declares no teleport emits no ledger at all, so a ledger that exists and says
//! zero is a finding rather than a pass.

use delvewright_dsl::stages::for_each_campaign_effect;

use crate::nav::NavError;
use crate::plan::Plan;
use delvewright_dsl::DwCode;

/// `DW0542`: a `teleport`'s source volume covers an interaction affordance the
/// engine has bound to hardware the teleport does not move (spec-0031).
///
/// One rule, one defect: *the compiler placed an entity and a block at the same
/// cell, and this verb moves only one of them.* The player is left looking at a
/// campfire, a lever or a sealed door that no longer answers a right-click —
/// visible, reachable, inert. It is the same silence `DW0426` and `DW0422` exist
/// to refuse, arriving from a third direction.
pub const DW_TELEPORT_BOUND_AFFORDANCE: DwCode = DwCode::every_version("DW0542");

/// The binding ledger for the teleport proof.
#[derive(Clone, Debug, Default)]
pub struct TeleportGate {
    /// `teleport` effects the campaign declared, at every nesting depth.
    pub declared: usize,
    /// Of those, the ones whose `from` volume resolved to a box on the solved
    /// layout. A gap is an unresolved anchor — already `DW0142`/`DW0360`,
    /// restated here so a reader of the ledger alone cannot mistake a dropped
    /// volume for a proven one.
    pub resolved: usize,
    /// World cells those volumes cover — the size of what the emitted selector
    /// sweeps.
    pub cells: usize,
    /// Engine affordances tested against every volume (`DW0542`).
    pub affordances: usize,
    /// PackTest templates generated for these teleports — the runtime half. The
    /// compile-time proof says the compiler wrote no filter; only a live server
    /// can say vanilla's `@e[<box>]` really reaches every entity type, and a
    /// compile-time-only green over a runtime mechanism is the vacuity this
    /// number exists to make visible. Filled by the emitter.
    pub packtests: usize,
}

impl TeleportGate {
    /// Whether this proof matched nothing at all.
    pub fn unbound(&self) -> bool {
        self.resolved == 0
    }

    /// The ledger as the `validation/teleport-gate.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "teleports": { "declared": self.declared, "resolved": self.resolved },
            "cells": self.cells,
            "affordances_examined": self.affordances,
            "packtest_templates": self.packtests,
            "unbound": self.unbound(),
        })
    }
}

/// One declared `teleport`, resolved onto the layout.
struct Volume {
    /// The effect's JSON pointer, for the diagnostic.
    path: String,
    /// The source volume's inclusive corners.
    lo: [i32; 3],
    /// The source volume's inclusive corners.
    hi: [i32; 3],
    /// The `from` anchor name, for the message.
    anchor: String,
}

impl Volume {
    /// Cells covered (inclusive on both corners) — what the ledger counts.
    fn cells(&self) -> usize {
        (0..3)
            .map(|i| (self.hi[i] - self.lo[i] + 1).max(0) as usize)
            .product()
    }

    /// Whether a cell lies inside the volume.
    fn contains(&self, p: [i32; 3]) -> bool {
        (0..3).all(|i| p[i] >= self.lo[i] && p[i] <= self.hi[i])
    }
}

/// Every declared `teleport`, in the deterministic order
/// [`for_each_campaign_effect`] visits effects (ADR-0006). An effect whose `from`
/// anchor does not resolve is counted as declared and dropped from the proof:
/// `check_effect_anchors` (`DW0360`) owns that failure and guessing a box here
/// would report a geometry defect for what is really a dangling reference.
fn volumes(plan: &Plan) -> (usize, Vec<Volume>) {
    let mut declared = 0usize;
    let mut out = Vec::new();
    for_each_campaign_effect(plan.campaign, &mut |path, _site, eff| {
        let Some((from, _to)) = eff.teleport() else {
            return;
        };
        declared += 1;
        if let Some((lo, hi)) = plan.zone_box(from) {
            out.push(Volume {
                path: path.to_string(),
                lo,
                hi,
                anchor: from.anchor.as_str().to_string(),
            });
        }
    });
    (declared, out)
}

/// Prove no `teleport` volume covers an engine affordance bound to hardware it
/// cannot move (`DW0542`), and return the binding ledger.
///
/// Empty and free for every campaign that declares no `teleport` — the walk
/// finds nothing, no affordance is enumerated, and the caller emits no ledger.
pub fn check_bound_affordances(plan: &Plan) -> Result<TeleportGate, NavError> {
    let (declared, vols) = volumes(plan);
    let mut gate = TeleportGate {
        declared,
        resolved: vols.len(),
        cells: vols.iter().map(Volume::cells).sum(),
        affordances: 0,
        packtests: 0,
    };
    if vols.is_empty() {
        return Ok(gate);
    }
    // `(what it is, which one, where)` — the affordance authority, plus the seal
    // shells it deliberately leaves to `DW0422`.
    let mut posts: Vec<(&'static str, String, [i32; 3])> = crate::eclipse::affordances(plan)
        .into_iter()
        .map(|a| {
            (
                a.kind,
                format!("`{}` at anchor `{}`", a.id, a.anchor),
                a.pos,
            )
        })
        .collect();
    for s in &plan.seal_hints {
        for cell in s.shell_cells() {
            posts.push((
                "sealed-gate answer",
                format!("the seal on anchor `{}`", s.anchor),
                cell,
            ));
        }
    }
    gate.affordances = posts.len();
    for v in &vols {
        for (kind, label, pos) in &posts {
            if !v.contains(*pos) {
                continue;
            }
            return Err(NavError {
                code: DW_TELEPORT_BOUND_AFFORDANCE,
                message: format!(
                    "the `teleport` at {} moves everything inside `{}` ± extent, and that volume \
                     covers the {kind} {label} at {pos:?}. The engine summons that affordance as \
                     an interaction entity standing on a block it also places; a teleport moves \
                     the entity and leaves the block, so the player is left with something they \
                     can see and reach that answers nothing. The selection is deliberately TOTAL \
                     — exempting `minecraft:interaction` the way a lethal volume must would tear \
                     an NPC's dialogue hitbox off its body, and everyone on the car travels \
                     (owner ruling 2026-08-08). Move the affordance out of the volume, or shrink \
                     the volume's `extent` so it does not cover it.",
                    v.path, v.anchor
                ),
            });
        }
    }
    Ok(gate)
}
