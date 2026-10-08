//! The ambush proof: an ambush leaves the player a play (`DW0376`).

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;
use std::collections::BTreeSet;

/// Prove every `ambush` (spec-0016 §3) leaves the player a play —
/// [`DW_AMBUSH_NO_COUNTERPLAY`] (`DW0376`).
///
/// The obligation is *not* "warn the player". Spec-0016 is explicit that the
/// un-telegraphed ambush — 初见杀, the shove off the cliff you could not have
/// known about — is legitimate and essential: you die uninformed once, and the
/// SECOND attempt is where the design pays off. Determinism already guarantees
/// that second attempt meets the same ambushers in the same cells.
///
/// What the compiler adds is the half determinism cannot supply: that there is
/// something to *do* about them. Generalizing the trap-avoidability machinery
/// (`DW0342`'s "reachable with the hazard cell blocked"), this stands every
/// ambusher on the cell it will occupy and re-asks whether the trigger cell still
/// connects to any rest point — a checkpoint, a bonfire, or the campaign entry.
/// If it does, a retreat exists: luring ground, a positioning line, an exit. If it
/// does not, the player is sealed in a pocket with the ambush and the beat has no
/// second attempt to reward — that is a broken beat, not a hard one.
pub fn check_ambushes(plan: &Plan, world: &World, entry: Option<[i32; 3]>) -> Result<(), Failure> {
    let mut rests: Vec<[i32; 3]> = plan.checkpoints.iter().map(|c| c.pos).collect();
    rests.extend(entry);
    verify_ambushes(world, &plan.ambushes, &rests)
}

/// The pure core of [`check_ambushes`] (unit-testable against a synthetic
/// [`World`]). `rests` are the cells that count as safety — every checkpoint and
/// bonfire cell plus the campaign entry.
fn verify_ambushes(
    world: &World,
    ambushes: &[crate::compiler::plan::AmbushPlan],
    rests: &[[i32; 3]],
) -> Result<(), Failure> {
    if ambushes.is_empty() || rests.is_empty() {
        return Ok(());
    }
    for amb in ambushes {
        let blocked: BTreeSet<[i32; 3]> = amb
            .actor_cells
            .iter()
            .copied()
            .filter(|c| *c != amb.at)
            .collect();
        if blocked.is_empty() {
            continue; // nothing stands in the player's way
        }
        let occupied = world.with_sealed(&blocked);
        let Some(from) = occupied.snap_standable(amb.at, SNAP_RADIUS) else {
            continue; // an unstandable trigger cell is another proof's concern
        };
        let escapes = rests.iter().any(|r| {
            occupied
                .snap_standable(*r, SNAP_RADIUS)
                .is_some_and(|goal| occupied.find_path(from, goal).is_some())
        });
        if !escapes {
            return Err(Failure {
                code: DW_AMBUSH_NO_COUNTERPLAY,
                message: format!(
                    "ambush `{}` at {:?} leaves no counterplay: with its ambushers standing on \
                     {:?}, no checkpoint, bonfire or campaign entry is walkable from the trigger \
                     cell any more — the party is sealed in a pocket with the ambush and can only \
                     trade blows blind. An un-telegraphed ambush is fine (spec-0016 §3: dying \
                     uninformed once is how the level teaches); an ambush with no retreat, no \
                     luring ground and no exit is not, because the second attempt has nothing to \
                     reward. Widen the room, move an ambusher off the only way out, or add a rest \
                     point behind the player — do NOT delete the proof.",
                    amb.id, amb.at, amb.actor_cells
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

    /// A 1-wide, ceilinged corridor along x at z=1 (walls at z=0 and z=2, floor
    /// at y=64, ceiling at y=67). A body standing in it cannot be climbed over —
    /// the headroom above a blocked cell is the ceiling.
    fn walled_corridor() -> World {
        let mut walls = Vec::new();
        for x in 0..9 {
            for y in [65, 66] {
                walls.push([x, y, 0]);
                walls.push([x, y, 2]);
            }
        }
        floored(9, 3, 65, &walls)
    }

    fn ambush(at: [i32; 3], actor_cells: Vec<[i32; 3]>) -> crate::compiler::plan::AmbushPlan {
        crate::compiler::plan::AmbushPlan {
            id: "ambush/stair-turn".to_string(),
            at,
            actor_cells,
        }
    }

    /// An ambush in an open room: the ambusher stands beside the player, so a
    /// retreat to the entry is still walkable. Un-telegraphed and lethal is fine
    /// — there is a play on the retry, which is all the engine owes.
    #[test]
    fn ambush_in_open_ground_has_counterplay() {
        let world = floored(9, 9, 65, &[]);
        let amb = ambush([4, 65, 4], vec![[5, 65, 4]]);
        verify_ambushes(&world, &[amb], &[[0, 65, 0]])
            .expect("an ambusher in open ground never seals the room");
    }

    /// A 1-wide corridor with the ambusher between the player and everything
    /// behind them: no retreat, no luring ground, no exit. `DW0376`.
    #[test]
    fn ambush_that_seals_the_only_way_out_is_dw0376() {
        let world = walled_corridor();
        // Player at the dead end (x=8); the ambusher spawns at x=7, behind them.
        let amb = ambush([8, 65, 1], vec![[7, 65, 1]]);
        let err = verify_ambushes(&world, &[amb], &[[0, 65, 1]])
            .expect_err("an ambush that corks the only corridor has no counterplay");
        assert_eq!(err.code, DW_AMBUSH_NO_COUNTERPLAY); // DW0376
        assert!(
            err.message.contains("no counterplay"),
            "the message must name the missing play: {}",
            err.message
        );
    }

    /// The same corridor, but a bonfire sits at the dead end WITH the player:
    /// dying is now cheap and the retry is a real second attempt, so the beat
    /// carries no obligation.
    #[test]
    fn ambush_with_a_rest_point_on_the_players_side_has_counterplay() {
        let world = walled_corridor();
        let amb = ambush([8, 65, 1], vec![[7, 65, 1]]);
        verify_ambushes(&world, &[amb], &[[0, 65, 1], [8, 65, 1]])
            .expect("a rest point on the player's own side is a play");
    }
}
