//! The shortcut proofs: a long route to the unlock exists, and opening the gate
//! shortens it (`DW0373`, `DW0374`).

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;
use std::collections::BTreeSet;

/// Prove every `shortcut` door (spec-0016 §2) is a real shortcut.
///
/// The base occupancy model treats every gate region as passable (the
/// "assume the gate the player needs is opened" stance), and `Plan::build`
/// registers each shortcut gate as sealed from step 0, so the critical path,
/// the checkpoints and the traps are all already proven **without** any shortcut
/// taken. What remains are the two obligations the pattern itself carries, both
/// measured against the same sealed world:
///
/// 1. [`DW_SHORTCUT_NO_LONG_ROUTE`] (`DW0373`) — with the gate SEALED, the
///    far-side `unlock` affordance must still be walkable from the campaign
///    entry. That walk IS the long route. Without it the mechanism that opens the
///    shortcut sits behind the shortcut, and the gate is dead scenery.
/// 2. [`DW_SHORTCUT_NO_GAIN`] (`DW0374`) — opening the gate must strictly shorten
///    that same walk. This is the anti-leak proof: it is what makes `unlock` a
///    FAR-side anchor rather than a label. An unlock on the near side of its own
///    gate measures identically sealed and open, and fails here.
///
/// Distances are A* step counts over the nav model — the same routing every other
/// completability proof uses, so the two numbers are directly comparable.
pub fn check_shortcuts(plan: &Plan, world: &World, entry: Option<[i32; 3]>) -> Result<(), Failure> {
    verify_shortcuts(world, &plan.shortcuts, entry)
}

/// The pure core of [`check_shortcuts`] (split out so it is unit-testable against
/// a synthetic [`World`] without a full [`Plan`]). With no resolvable entry cell
/// there is nothing to measure from and both proofs are vacuous — `DW0345`
/// already fails a campaign whose entry does not resolve.
fn verify_shortcuts(
    world: &World,
    shortcuts: &[crate::compiler::plan::ShortcutPlan],
    entry: Option<[i32; 3]>,
) -> Result<(), Failure> {
    let Some(entry) = entry else {
        return Ok(());
    };
    for sc in shortcuts {
        let cells: BTreeSet<[i32; 3]> =
            crate::compiler::assembled::region_cells(sc.gate_region.0, sc.gate_region.1).collect();
        let sealed = world.with_sealed(&cells);

        // Both walks are measured from the same footing and to the same goal; only
        // the gate differs. Snapping happens on the SEALED world so neither
        // endpoint can land inside the gate region itself.
        let start = sealed.snap_standable(entry, SNAP_RADIUS);
        let goal = sealed.snap_standable(sc.unlock, SNAP_RADIUS);
        let (Some(start), Some(goal)) = (start, goal) else {
            // An unstandable entry or unlock is another proof's concern
            // (`DW0345` / the anchor checks); do not double-report it here.
            continue;
        };

        // (1) the long route exists while the gate is sealed.
        let Some(long) = sealed.find_path(start, goal).map(|p| p.len()) else {
            return Err(Failure {
                code: DW_SHORTCUT_NO_LONG_ROUTE,
                message: format!(
                    "shortcut `{}`: no long route — its unlock affordance at `{}` ({:?}) is not \
                     walkable from the campaign entry while gate `{}` is sealed, so the mechanism \
                     that opens the shortcut sits behind the shortcut and can never be pulled. A \
                     shortcut is earned the hard way first (spec-0016 §2): connect the far side by \
                     a long route, or move the unlock onto one. Do NOT open the gate at world-load \
                     to silence this.",
                    sc.id, sc.unlock_anchor, sc.unlock, sc.gate_anchor
                ),
            });
        };

        // (2) opening the gate strictly shortens that same walk (anti-leak).
        let short = world
            .find_path(start, goal)
            .map(|p| p.len())
            .unwrap_or(long);
        if short >= long {
            return Err(Failure {
                code: DW_SHORTCUT_NO_GAIN,
                message: format!(
                    "shortcut `{}` leaks: opening gate `{}` does not shorten the walk from the \
                     campaign entry to its own unlock `{}` ({long} steps sealed, {short} open), so \
                     the unlock is not on the far side of anything and the loop-back the shortcut \
                     is FOR never happens. Put the unlock past the gate, on the end of the long \
                     route (spec-0016 §2) — never delete the proof.",
                    sc.id, sc.gate_anchor, sc.unlock_anchor
                ),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::*;

    /// A shortcut plan over a 1-cell gate column at `(gx, y..y+1, zw)`.
    fn shortcut(gx: i32, y: i32, zw: i32, unlock: [i32; 3]) -> crate::compiler::plan::ShortcutPlan {
        crate::compiler::plan::ShortcutPlan {
            id: "shortcut/lift".to_string(),
            safe: "lift".to_string(),
            gate_anchor: "anchor/gate".to_string(),
            gate_region: ([gx, y, zw], [gx, y + 1, zw]),
            gate_block: "minecraft:iron_bars".to_string(),
            unlock_anchor: "anchor/lift-lever".to_string(),
            unlock,
            on_unlock: Vec::new(),
            sealed_side: crate::compiler::wrongside::derive(([gx, y, zw], [gx, y + 1, zw]), unlock),
        }
    }

    /// The happy path: a wall with a barred doorway AND a far bypass hole. The
    /// unlock is reachable the long way while the gate is sealed, and opening the
    /// gate genuinely shortens the crossing — a real shortcut.
    #[test]
    fn shortcut_with_a_long_way_round_passes_both_proofs() {
        let world = shortcut_world(12, 9, 65, 4, 1, Some(10));
        let sc = shortcut(1, 65, 4, [1, 65, 7]);
        verify_shortcuts(&world, &[sc], Some([1, 65, 1]))
            .expect("a gate with a genuine detour around it is a real shortcut");
    }

    /// No bypass: sealing the gate cuts the room in two, so the unlock on the far
    /// side can never be reached the hard way — the mechanism that opens the
    /// shortcut is behind the shortcut. `DW0373`.
    #[test]
    fn shortcut_whose_unlock_is_only_behind_its_own_gate_is_dw0373() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let sc = shortcut(1, 65, 4, [1, 65, 7]);
        let err = verify_shortcuts(&world, &[sc], Some([1, 65, 1]))
            .expect_err("a shortcut with no long route must fail");
        assert_eq!(err.code, DW_SHORTCUT_NO_LONG_ROUTE); // DW0373
        assert!(
            err.message.contains("no long route"),
            "the message must name the missing long route: {}",
            err.message
        );
    }

    /// The classic leak: the `unlock` sits on the NEAR side of its own gate, so
    /// the player can pull it without ever earning the far side — and opening the
    /// gate measurably changes nothing about reaching it. `DW0374`.
    #[test]
    fn shortcut_whose_unlock_is_on_the_near_side_is_dw0374() {
        let world = shortcut_world(12, 9, 65, 4, 1, Some(10));
        // Entry z=1, wall z=4: an unlock at z=2 is on the entry's own side.
        let sc = shortcut(1, 65, 4, [5, 65, 2]);
        let err = verify_shortcuts(&world, &[sc], Some([1, 65, 1]))
            .expect_err("an unlock the gate does not stand in front of is a leak");
        assert_eq!(err.code, DW_SHORTCUT_NO_GAIN); // DW0374
        assert!(
            err.message.contains("leaks"),
            "the message must name the leak: {}",
            err.message
        );
    }
}
