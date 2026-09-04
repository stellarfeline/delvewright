//! Crosshair-disambiguation proof: two things the party has to click may not
//! stand close enough that the crosshair cannot tell them apart (`DW0489`).
//!
//! ## The defect this exists for (owner playtest, island — terminal finding)
//!
//! Two crew NPCs stood at the cave mouth. The player could not put the crosshair
//! on the right one, so the dialogue that carries the take-or-wait decision never
//! opened: a hard soft-lock, with the whole machine ladder **green**. It was
//! green because the ladder's bot interacts by *entity id* — it never casts the
//! ray a real player casts, so occlusion is invisible to it (the harness half of
//! this fix, `harness/src/crosshair.ts`, is the other end of the same defect).
//!
//! The campaign's own `quests.json` states the fault outright:
//! `quest/follow-the-smoke` declares `npc/eurylochus` **and** `npc/antiphos`
//! both `at: anchor/mouth`. Two 0.6-wide mannequin bodies on one cell, 0.00
//! blocks apart, in both branch worlds. Nothing was looking.
//!
//! ## Why the existing proofs cannot see it
//!
//! [`crate::eclipse`] (`DW0359`) is the closest rule — a body in front of a
//! *thing to click* — and it is structurally blind here for two reasons, both
//! deliberate:
//!
//! * it compares a body against an **affordance**, never a body against another
//!   **body**; an NPC's own dialogue hitbox is not in its affordance list;
//! * it applies the **parked-body rule** — any NPC the campaign ever `move-npc`s
//!   is skipped entirely, because a declared anchor is only a walker's starting
//!   mark and deciding "is it still there when this goes live?" needs a timeline
//!   it will not guess. `npc/eurylochus` and `npc/antiphos` are both walkers, so
//!   `DW0359` never looked at either of them.
//!
//! That timeline is no longer a guess. DSL v0.7's **cast ledger**
//! ([`crate::cast`], spec-0020) makes every quest declare, for every live NPC,
//! exactly where it stands — and `DW0461` already proves that declaration equals
//! the position the effect history produces. So the ledger is a *checked*
//! roster of who is on stage together, beat by beat, and this module is the
//! geometry over it. No inference, no invented certainty: the campaign said it.
//!
//! ## The model
//!
//! * **Scene.** One quest, one branch world. Two placements share a scene when
//!   they are declared in the same quest and no flag proves them mutually
//!   exclusive (one's `requires_flags` meeting the other's `forbids_flags`).
//!   Anything less than a proof of exclusivity is co-presence — the direction
//!   that can only ever withhold a diagnostic, never invent one.
//! * **Target.** An NPC on stage wears an `Invulnerable` body plus a co-located
//!   `minecraft:interaction` box: right-click is how the party reaches it, so it
//!   is a crosshair target. Width comes from [`crate::nav::entity_dims`] over
//!   the body that actually ships ([`crate::nav::npc_body_entity`] — a skinned
//!   NPC is a 0.6-wide `minecraft:mannequin`, not its declared base entity).
//! * **Contest.** Two targets in one scene whose horizontal centre separation is
//!   below [`threshold`], with overlapping vertical spans.
//! * **Vertical escape.** Spans that do not overlap are silent: the crosshair
//!   separates them by aiming up or down, from every azimuth, and no horizontal
//!   distance is needed at all.
//!
//! ## The threshold, derived (vanilla 1.21.11 geometry only)
//!
//! Vanilla picks an entity by ray: `GameRenderer.pick` traces from the eye along
//! the look vector out to `player.entity_interaction_range`
//! ([`INTERACTION_REACH`] = 3.0 blocks), and `ProjectileUtil.getEntityHitResult`
//! returns the entity whose bounding box the ray meets **first**. The box is
//! inflated by `Entity.getPickRadius()`, which is `0.0` for every body a delve
//! stages — so the contest is pure box geometry with no tolerance to hide in.
//!
//! Take a target `t` and another body `o`, horizontal centre separation `s`.
//!
//! 1. The player is a body too: [`PLAYER_WIDTH`] = 0.6 blocks. Its eye can never
//!    be nearer than `(PLAYER_WIDTH + w_t)/2` to `t`'s centre — it would be
//!    standing inside `t`.
//! 2. `o` can only steal the click if it lies between the eye and `t`. It
//!    provably cannot, from *any* azimuth, when the eye is nearer to `t` than
//!    `o`'s near face is: `d < s - (w_t + w_o)/2`.
//! 3. Such a stance therefore exists exactly when
//!    `(PLAYER_WIDTH + w_t)/2 < s - (w_t + w_o)/2`.
//!
//! Rearranged, and symmetrised over which of the two is the target (the wider
//! body is the harder case), that is [`threshold`]:
//!
//! ```text
//! τ(t, o) = (PLAYER_WIDTH + max(w_t, w_o)) / 2  +  (w_t + w_o) / 2
//! ```
//!
//! For two 0.6-wide humanoid bodies τ = 0.6 + 0.6 = **1.2 blocks**. The stance it
//! guarantees sits 0.6 blocks from the target — far inside the 3.0-block reach,
//! so "provably clear" and "close enough to click" are never in tension.
//!
//! At or above τ there is a standing position, reachable and legal, from which
//! the target is unambiguously the first thing the ray meets. Below τ every
//! stance that can reach the target lies at or beyond the other body's near face
//! on some azimuths; whether a clear azimuth survives depends on the walls and
//! floor around the pair, which this proof does not model — which is precisely
//! how the island's two crew ended up unclickable while every check was green.
//! `s = 0` is the degenerate worst case: coincident boxes make the ray-pick an
//! exact tie that the client resolves by entity iteration order, so *which* NPC
//! answers a right-click is not decidable from the campaign at all.
//!
//! ## Two tiers, one code
//!
//! **Error (exit 3)** when either placement's right-click opens a dialogue
//! **root** — the ledger's own word for a consequential tree, which is where
//! every `talk-to` objective and every branch choice lives. A click lost there
//! costs the party a beat they cannot get back.
//!
//! **Warning (exit 0)** when both placements are barks or silent. The bodies are
//! just as ambiguous, but nothing is riding on the click, so this is a staging
//! note for the owner's QA hour rather than a build failure.
//!
//! ## Prescription, and the fix that is never prescribed
//!
//! Move one of the two cast anchors. **Never** make a body intangible or
//! non-pickable to let clicks pass through it — [`crate::eclipse`] states the
//! same rule for the same reason: a body the party cannot click is a character
//! they cannot talk to, which trades one dead beat for another.
//!
//! ## Known boundary
//!
//! Only NPC-vs-NPC, only over the cast ledger. Body-vs-affordance at rest
//! belongs to `DW0359` and is not re-litigated here (one code, one rule); actors
//! carry no ledger entry, so a puppet parked in front of a speaker is still
//! nobody's rule.

use crate::failure::Failure;
use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{CastDialogue, CastPlacement, Diagnostic, Npc};

use crate::nav::{entity_dims, npc_body_entity};
use crate::plan::Plan;
use delvewright_dsl::DwCode;

/// `DW0489`: two crosshair targets stand close enough that a player cannot aim
/// at one without risking the other.
pub const DW_CROSSHAIR_CONTEST: DwCode = DwCode::every_version("DW0489");

/// The vanilla player hitbox width (1.21.11), in blocks. The player is a body:
/// its eye can never be nearer than `(PLAYER_WIDTH + w)/2` to another body's
/// centre, which is what bounds the closest stance in [`threshold`].
///
/// The metrics table (spec-0049 §2) is the one definition; this re-export keeps
/// the name every call site already reads while removing the second copy.
pub use delvewright_dsl::metrics::PLAYER_WIDTH;

/// `player.entity_interaction_range`, the vanilla 1.21.11 default, in blocks —
/// how far the entity-pick ray is traced from the eye.
///
/// It does not enter [`threshold`] as a term: the guaranteed-clear stance the
/// threshold constructs is always well inside it. It is documented here because
/// it is what makes that stance *usable* — a disambiguation rule that put the
/// player 4 blocks out would have separated the bodies and lost the click.
pub const INTERACTION_REACH: f64 = 3.0;

/// The minimum horizontal centre separation at which a player can always find a
/// stance where `w_target`'s box is the first thing the pick ray meets, with
/// `w_other` provably not between. See the module docs for the derivation.
pub fn threshold(w_target: f64, w_other: f64) -> f64 {
    (PLAYER_WIDTH + w_target.max(w_other)) / 2.0 + (w_target + w_other) / 2.0
}

/// One NPC on stage in one scene: a crosshair target with a body and a place.
struct Target<'a> {
    /// The declaring id (`npc/eurylochus`).
    id: &'a str,
    /// The entity whose hitbox the body wears (`minecraft:mannequin`).
    entity: String,
    /// The body's horizontal width, in blocks.
    width: f64,
    /// The body's height, in blocks.
    height: f64,
    /// The cast anchor this placement declares.
    anchor: &'a str,
    /// The resolved anchor cell (feet).
    cell: [i32; 3],
    /// Whether right-click opens a consequential dialogue root (the error tier).
    root: Option<&'a str>,
    /// The branch gate this placement carries.
    requires: BTreeSet<&'a str>,
    /// The negative branch gate this placement carries.
    forbids: BTreeSet<&'a str>,
}

impl Target<'_> {
    /// The horizontal centre of the body, in world coordinates — `emit`'s
    /// `ent_xyz`, the cell centre the `summon`/`tp` actually carries.
    fn centre(&self) -> (f64, f64) {
        (self.cell[0] as f64 + 0.5, self.cell[2] as f64 + 0.5)
    }

    /// The vertical span the body occupies: `height` rising from the cell floor.
    fn vertical(&self) -> (f64, f64) {
        (self.cell[1] as f64, self.cell[1] as f64 + self.height)
    }

    /// ``npc `npc/eurylochus` (minecraft:mannequin, 0.6 × 1.8 blocks)``.
    fn describe(&self) -> String {
        format!(
            "npc `{}` ({}, {} × {} blocks)",
            self.id, self.entity, self.width, self.height
        )
    }
}

/// Whether two placements can be on stage at once.
///
/// They provably cannot only when one's `requires_flags` names a flag the
/// other's `forbids_flags` names — the campaign stating the two describe
/// different branches. Everything else is co-presence, because the ledger says
/// both are in this quest and nothing says otherwise. This is the
/// no-false-certainty direction: an unprovable exclusion withholds nothing but a
/// diagnostic the geometry has already earned.
fn co_present(a: &Target<'_>, b: &Target<'_>) -> bool {
    a.requires.intersection(&b.forbids).next().is_none()
        && b.requires.intersection(&a.forbids).next().is_none()
}

/// The dialogue root a placement's right-click opens, if it opens one.
///
/// `barks` and `"none"` are the ledger's declarations that the click is
/// inconsequential; `"unchanged"` carries a previous scene forward and is not
/// re-judged here (whatever it carries was judged at the quest that declared
/// it).
fn root_of(p: &CastPlacement) -> Option<&str> {
    match p.dialogue.as_ref()? {
        CastDialogue::Root(r) => Some(r.as_str()),
        _ => None,
    }
}

/// Every NPC on stage in one quest, resolved to a body at a cell.
///
/// A placement whose anchor does not resolve is skipped: `DW0464`/`DW0461` own
/// dangling and contradicted cast anchors, and reporting a geometry defect for
/// what is really a bad reference would name the wrong bug.
fn scene<'a>(plan: &Plan<'a>, quest: &'a delvewright_dsl::Quest) -> Vec<Target<'a>> {
    let npcs: BTreeMap<&str, &Npc> = plan
        .campaign
        .npcs
        .content
        .npcs
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();
    let mut out = Vec::new();
    for (npc_id, entry) in &quest.cast {
        let Some(npc) = npcs.get(npc_id.as_str()) else {
            continue;
        };
        for p in entry.placements() {
            let Some(anchor) = p.at.anchor() else {
                continue; // `offstage` / `dead`: no body to click.
            };
            let Some(cell) = plan
                .point(npc.area.as_str(), anchor.as_str())
                .or_else(|| plan.point_any(anchor.as_str()))
            else {
                continue;
            };
            let entity = npc_body_entity(npc);
            let (width, height) = entity_dims(&entity);
            out.push(Target {
                id: npc_id.as_str(),
                entity,
                width,
                height,
                anchor: anchor.as_str(),
                cell,
                root: root_of(p),
                requires: p.requires_flags.iter().map(|f| f.as_str()).collect(),
                forbids: p.forbids_flags.iter().map(|f| f.as_str()).collect(),
            });
        }
    }
    out
}

/// Prove that no two crosshair targets contest one click (`DW0489`).
///
/// Returns the warning-tier contests on success; the first error-tier contest —
/// one where a consequential dialogue root is at stake — fails the build. Both
/// are produced in quest-DAG order then cast declaration order, so the report is
/// deterministic (ADR-0006).
///
/// Empty for every campaign whose staged NPCs keep [`threshold`] apart, and for
/// every campaign with no cast ledger at all (pre-0.7), so output stays
/// byte-identical.
pub fn check_crosshair_contests(plan: &Plan) -> Result<Vec<Diagnostic>, Failure> {
    let c = plan.campaign;
    let mut warnings = Vec::new();
    for qid in crate::cast::quest_dag_order(c) {
        let Some((qi, quest)) = c
            .quests
            .content
            .quests
            .iter()
            .enumerate()
            .find(|(_, q)| q.id.as_str() == qid)
        else {
            continue;
        };
        let targets = scene(plan, quest);
        // Every unordered pair, in declaration order — one report per pair, not
        // one per direction.
        for (i, a) in targets.iter().enumerate() {
            for b in targets.iter().skip(i + 1) {
                if a.id == b.id || !co_present(a, b) {
                    continue;
                }
                let (ax, az) = a.centre();
                let (bx, bz) = b.centre();
                let sep = ((ax - bx).powi(2) + (az - bz).powi(2)).sqrt();
                let tau = threshold(a.width, b.width);
                if sep >= tau {
                    continue;
                }
                let (alo, ahi) = a.vertical();
                let (blo, bhi) = b.vertical();
                if alo >= bhi || blo >= ahi {
                    // No shared vertical band: aim up or down and the two are
                    // separated from every azimuth.
                    continue;
                }
                let path = format!("/content/quests/{qi}/cast/{}", a.id);
                match (a.root, b.root) {
                    (None, None) => warnings.push(Diagnostic::warning(
                        DW_CROSSHAIR_CONTEST,
                        "quests",
                        path,
                        message(&qid, a, b, sep, tau, false),
                    )),
                    _ => {
                        return Err(Failure {
                            code: DW_CROSSHAIR_CONTEST,
                            message: message(&qid, a, b, sep, tau, true),
                        });
                    }
                }
            }
        }
    }
    Ok(warnings)
}

/// The `DW0489` message, in both tiers: name both bodies, the scene, the
/// measurement against the derived threshold, and the one prescription.
fn message(
    quest: &str,
    a: &Target<'_>,
    b: &Target<'_>,
    sep: f64,
    tau: f64,
    flow_critical: bool,
) -> String {
    let coincident = sep < 1e-9;
    let geometry = if coincident {
        "Their hitboxes are COINCIDENT: the client's entity ray-pick is an exact tie and resolves \
         by iteration order, so which of the two answers a right-click is not decidable from the \
         campaign at all."
            .to_string()
    } else {
        format!(
            "Their centres are {sep:.2} blocks apart, closer than the {tau:.2} blocks a player \
             needs to be able to stand nearer the one they want than the other's near face — below \
             that, every stance within interaction reach ({INTERACTION_REACH} blocks) puts the \
             second body between eye and target on some approach azimuths."
        )
    };
    let stake = if flow_critical {
        let root = a.root.or(b.root).unwrap_or("");
        format!(
            "One of them opens the dialogue root `{root}`, which the cast ledger declares as a \
             consequential tree — a click that lands on the wrong body loses a beat the party \
             cannot get back, and that is how a delve soft-locks with every other proof green."
        )
    } else {
        "Neither right-click is consequential (barks or `none`), so this is a staging measurement \
         for playtest rather than a build failure — advisory precisely because nothing is riding \
         on which body answers."
            .to_string()
    };
    format!(
        "in `{quest}`, {} stands at `{}` {:?} and {} stands at `{}` {:?} — both on stage in the \
         same scene, and both are things the party clicks. {geometry} {stake} Threshold derived \
         from vanilla 1.21.11 geometry alone: the player body is {PLAYER_WIDTH} blocks wide, so \
         the eye cannot come nearer than half that plus half the target to its centre, and the \
         other body is provably out of the way only from nearer still. Prescription: move one of \
         the two cast anchors so the scene keeps {tau:.2} blocks between them. Do NOT make either \
         body intangible or non-pickable to let clicks through — a body the party cannot click is \
         a character they cannot talk to.",
        a.describe(),
        a.anchor,
        a.cell,
        b.describe(),
        b.anchor,
        b.cell,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The derivation, at the size every staged humanoid actually is.
    #[test]
    fn two_humanoids_need_one_point_two_blocks() {
        assert!((threshold(0.6, 0.6) - 1.2).abs() < 1e-9);
    }

    /// The wider body is the harder case, and the rule is symmetric in the pair.
    #[test]
    fn threshold_is_symmetric_and_grows_with_the_wider_body() {
        assert!((threshold(0.9, 0.6) - threshold(0.6, 0.9)).abs() < 1e-9);
        assert!(threshold(0.9, 0.6) > threshold(0.6, 0.6));
        // warden (0.9) beside a mannequin (0.6): (0.6+0.9)/2 + (0.9+0.6)/2.
        assert!((threshold(0.9, 0.6) - 1.5).abs() < 1e-9);
    }

    /// The stance the threshold guarantees is always inside interaction reach —
    /// the property that keeps "provably clear" and "close enough to click" from
    /// pulling against each other.
    #[test]
    fn the_guaranteed_stance_is_within_reach() {
        for (wt, wo) in [(0.6, 0.6), (0.9, 0.6), (1.4, 0.9), (1.95, 1.95)] {
            let stance = (PLAYER_WIDTH + wt) / 2.0;
            assert!(
                stance < INTERACTION_REACH,
                "stance {stance} for widths {wt}/{wo} escapes reach"
            );
            assert!(threshold(wt, wo) > 0.0);
        }
    }

    fn target<'a>(id: &'a str, cell: [i32; 3], root: Option<&'a str>) -> Target<'a> {
        Target {
            id,
            entity: "minecraft:mannequin".into(),
            width: 0.6,
            height: 1.8,
            anchor: "anchor/mouth",
            cell,
            root,
            requires: BTreeSet::new(),
            forbids: BTreeSet::new(),
        }
    }

    /// The island's shape: two mannequins declared on one cell.
    #[test]
    fn coincident_bodies_are_a_contest() {
        let a = target("npc/eurylochus", [9, 69, -43], Some("dlg/root"));
        let b = target("npc/antiphos", [9, 69, -43], Some("dlg/at-the-mouth"));
        let (ax, az) = a.centre();
        let (bx, bz) = b.centre();
        let sep = ((ax - bx).powi(2) + (az - bz).powi(2)).sqrt();
        assert!(sep < threshold(a.width, b.width));
        assert!(message("quest/follow-the-smoke", &a, &b, sep, 1.2, true).contains("COINCIDENT"));
    }

    /// One cell of separation is still inside the threshold; two is clear.
    #[test]
    fn one_cell_apart_contests_and_two_cells_do_not() {
        let a = target("npc/a", [0, 64, 0], None);
        let one = target("npc/b", [1, 64, 0], None);
        let two = target("npc/c", [2, 64, 0], None);
        let tau = threshold(0.6, 0.6);
        assert!(1.0 < tau, "adjacent cells must contest");
        let (ax, az) = a.centre();
        for (t, contest) in [(&one, true), (&two, false)] {
            let (bx, bz) = t.centre();
            let sep = ((ax - bx).powi(2) + (az - bz).powi(2)).sqrt();
            assert_eq!(sep < tau, contest, "separation {sep} vs threshold {tau}");
        }
    }

    /// A branch gate that provably separates two placements is not co-presence.
    #[test]
    fn opposed_branch_gates_are_not_co_present() {
        let mut a = target("npc/a", [0, 64, 0], None);
        let mut b = target("npc/b", [0, 64, 0], None);
        a.requires.insert("flag/flee");
        b.forbids.insert("flag/flee");
        assert!(!co_present(&a, &b));
        // …but a gate that merely differs is still co-presence: the ledger has
        // not proven the two describe different worlds.
        let mut c = target("npc/c", [0, 64, 0], None);
        c.requires.insert("flag/wait");
        assert!(co_present(&a, &c));
    }

    /// Bodies stacked on different floors share no vertical band.
    #[test]
    fn vertical_separation_needs_no_horizontal_distance() {
        let low = target("npc/a", [0, 64, 0], None);
        let high = target("npc/b", [0, 70, 0], None);
        let (llo, lhi) = low.vertical();
        let (hlo, hhi) = high.vertical();
        assert!(llo >= hhi || hlo >= lhi);
    }

    /// The code is the one the reference documents.
    #[test]
    fn code_is_dw0489() {
        assert_eq!(DW_CROSSHAIR_CONTEST, "DW0489");
    }
}
