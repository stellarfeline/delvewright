//! **A trap's trigger is a block the party sees and springs, so it has to be
//! there** (`DW0917`).
//!
//! A trap's `trigger` names a kind of hardware — a pressure plate, a tripwire,
//! a trapped chest — and the prefab places it, exactly as it places a trap's
//! dispenser or a `loot` container. The compiler never places a trigger. What
//! it emits at the trigger cell is detection: a position test for a plate or a
//! tripwire, an invisible `minecraft:interaction` hitbox for a trapped chest.
//! Neither needs the block to exist, so a trap whose cell holds air still
//! compiles, still passes its completability proofs (they reason about the
//! planned hazard) and still fires — on a cell that shows the player nothing.
//! A trapped-chest trap ships as a click on empty air; a plate trap ships as an
//! invisible hazard. The direction of that gap is the dangerous one: it let a
//! broken trap ship, it could never turn a proof red.
//!
//! So the question is asked of the assembled world (the edited one when a
//! stage-7 script exists, since a batch may be what places the chest), in the
//! same pass and over the same block map as the container proofs.

use std::collections::BTreeMap;

use crate::compiler::assembled::base_id;
use crate::compiler::failure::Failure;
use crate::compiler::plan::{ResolvedAnchor, TrapPlan};
use delvewright_dsl::{DwCode, ExitTier};

delvewright_dsl::dw_code! {
    /// A cell the compiler detects a step or an opening on does not hold the
    /// block that invites it: a trap's trigger cell without the block its
    /// `trigger` kind names, or a `step` trigger's cell without a block a step
    /// fires.
    pub const DW_TRAP_TRIGGER_MISSING: DwCode = DwCode::new("DW0917", ExitTier::Build);
}

/// Build-tier proof: every trap's trigger cell holds the block its trigger kind
/// names. `anchors` is only read for the remedy: the anchors of this world
/// whose cell already holds a matching block.
pub fn check_trap_triggers(
    blocks: &crate::compiler::blockstate::BlockMap,
    traps: &[TrapPlan],
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Result<(), Failure> {
    let at =
        |c: &[i32; 3]| -> &str { blocks.get(c).map(|s| s.as_str()).unwrap_or("minecraft:air") };
    let mut bad: Vec<String> = Vec::new();
    for t in traps {
        let c = t.trigger_cell;
        let found = at(&c);
        if t.trigger.is_trigger_block(found) {
            continue;
        }
        let elsewhere: Vec<String> = anchors
            .iter()
            .filter_map(|((area, name), r)| match r {
                ResolvedAnchor::Point { pos, .. } if t.trigger.is_trigger_block(at(pos)) => {
                    Some(format!("`{name}` in `{area}`"))
                }
                _ => None,
            })
            .collect();
        let remedy = if elsewhere.is_empty() {
            "no anchor of this assembled world holds one".to_string()
        } else {
            format!("anchors that hold one: {}", elsewhere.join(", "))
        };
        bad.push(format!(
            "  trap `{}` ({}) -> anchor `{}` at [{}, {}, {}] holds `{}`, not {}; {remedy}",
            t.id,
            t.trigger.kind(),
            t.at_anchor,
            c[0],
            c[1],
            c[2],
            base_id(found),
            t.trigger.trigger_block_name(),
        ));
    }
    if bad.is_empty() {
        return Ok(());
    }
    Err(Failure {
        code: DW_TRAP_TRIGGER_MISSING,
        message: format!(
            "{} trap(s) have no trigger block at their trigger cell.\n{}\n\
             A trap's trigger is hardware the PREFAB places, exactly as a trap's dispenser \
             is; the compiler only detects it. Detection does not need the block, so this \
             would have shipped a trap the player cannot see: a click on empty air where the \
             chest should be, or an invisible plate. Point the trap's `at` at an anchor whose \
             cell holds the trigger block, or have the piece place the block at this anchor's \
             cell (a prefab-library change). Do NOT reach for a `set-block` effect to place \
             it at runtime.",
            bad.len(),
            bad.join("\n"),
        ),
    })
}

/// One `step` trigger as [`check_step_triggers`] judges it: its id, its anchor,
/// and the cell that anchor resolves to.
pub struct StepCell<'a> {
    /// The trigger's id.
    pub id: &'a str,
    /// The anchor the trigger watches.
    pub anchor: &'a str,
    /// The anchor's cell in the assembled world.
    pub cell: [i32; 3],
}

/// Build-tier proof, the `step` half of `DW0917`: every `step` trigger's cell
/// holds a block a step fires ([`delvewright_dsl::fires_on_step`]). The
/// compiler's detection is a player in the cell, which needs no block, so a
/// cell holding air would ship a beat fired by stepping on bare floor that
/// shows the player nothing.
pub fn check_step_triggers(
    blocks: &crate::compiler::blockstate::BlockMap,
    steps: &[StepCell<'_>],
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Result<(), Failure> {
    let at =
        |c: &[i32; 3]| -> &str { blocks.get(c).map(|s| s.as_str()).unwrap_or("minecraft:air") };
    let mut bad: Vec<String> = Vec::new();
    for t in steps {
        let found = at(&t.cell);
        if delvewright_dsl::fires_on_step(found) {
            continue;
        }
        let elsewhere: Vec<String> = anchors
            .iter()
            .filter_map(|((area, name), r)| match r {
                ResolvedAnchor::Point { pos, .. } if delvewright_dsl::fires_on_step(at(pos)) => {
                    Some(format!("`{name}` in `{area}`"))
                }
                _ => None,
            })
            .collect();
        let remedy = if elsewhere.is_empty() {
            "no anchor of this assembled world holds one".to_string()
        } else {
            format!("anchors that hold one: {}", elsewhere.join(", "))
        };
        bad.push(format!(
            "  trigger `{}` (step) -> anchor `{}` at [{}, {}, {}] holds `{}`, not a pressure \
             plate or a tripwire string; {remedy}",
            t.id,
            t.anchor,
            t.cell[0],
            t.cell[1],
            t.cell[2],
            base_id(found),
        ));
    }
    if bad.is_empty() {
        return Ok(());
    }
    Err(Failure {
        code: DW_TRAP_TRIGGER_MISSING,
        message: format!(
            "{} `step` trigger(s) have no plate or tripwire at their cell.\n{}\n\
             A `step` trigger fires on a player standing in its cell, and the block that \
             tells the player to step there is hardware the PREFAB places; the compiler only \
             detects the step. Detection does not need the block, so this would have shipped \
             a beat fired by bare floor. Point the trigger's `at` at an anchor whose cell holds \
             a pressure plate or a tripwire string, or have the piece place one at this \
             anchor's cell (a prefab-library change). Do NOT reach for a `set-block` effect to \
             place it at runtime.",
            bad.len(),
            bad.join("\n"),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use delvewright_dsl::{Lethality, TrapReset, TrapTrigger};

    fn step(cell: [i32; 3]) -> StepCell<'static> {
        StepCell {
            id: "trigger/doormat",
            anchor: "anchor/march",
            cell,
        }
    }

    /// A `step` trigger over bare floor is refused, naming the trigger and the
    /// cell; over a plate or a tripwire it is not.
    #[test]
    fn a_step_trigger_over_air_is_dw0917() {
        let e = check_step_triggers(&BTreeMap::new(), &[step([4, 1, 7])], &BTreeMap::new())
            .unwrap_err();
        assert_eq!(e.code, "DW0917");
        assert!(e.message.contains("trigger/doormat"), "{}", e.message);
        assert!(e.message.contains("[4, 1, 7]"), "{}", e.message);
        for block in [
            "minecraft:stone_pressure_plate",
            "minecraft:light_weighted_pressure_plate[power=0]",
            "minecraft:tripwire[attached=true]",
        ] {
            assert!(
                check_step_triggers(
                    &crate::compiler::blockstate::interned(world([1, 2, 3], block)),
                    &[step([1, 2, 3])],
                    &BTreeMap::new()
                )
                .is_ok(),
                "{block}"
            );
        }
        let hook = check_step_triggers(
            &crate::compiler::blockstate::interned(world([1, 2, 3], "minecraft:tripwire_hook")),
            &[step([1, 2, 3])],
            &BTreeMap::new(),
        );
        assert_eq!(hook.unwrap_err().code, "DW0917", "a hook is not stepped on");
    }

    fn trap(trigger: TrapTrigger, cell: [i32; 3]) -> TrapPlan {
        TrapPlan {
            id: "trap/false-chest".to_string(),
            safe: "false_chest".to_string(),
            trigger,
            at_anchor: "anchor/strongbox".to_string(),
            trigger_cell: cell,
            dispenser: None,
            payload: None,
            payload_effects: Vec::new(),
            lethality: Lethality::Nonlethal,
            reset: TrapReset::Once,
            disarm: None,
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
        }
    }

    fn world(cell: [i32; 3], block: &str) -> BTreeMap<[i32; 3], String> {
        let mut b = BTreeMap::new();
        b.insert(cell, block.to_string());
        b
    }

    /// The shipped defect: a trapped-chest trap over empty air.
    #[test]
    fn a_trapped_chest_trap_over_air_is_dw0917() {
        let e = check_trap_triggers(
            &BTreeMap::new(),
            &[trap(TrapTrigger::TrappedChest, [4, 1, 7])],
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert_eq!(e.code, "DW0917");
        assert!(e.message.contains("minecraft:air"), "{}", e.message);
        assert!(e.message.contains("[4, 1, 7]"), "{}", e.message);
        assert!(e.message.contains("trap/false-chest"), "{}", e.message);
    }

    /// A plain chest is not the trigger: it does not spring anything.
    #[test]
    fn a_plain_chest_is_not_a_trapped_chest() {
        let e = check_trap_triggers(
            &crate::compiler::blockstate::interned(world(
                [0, 0, 0],
                "minecraft:chest[facing=north]",
            )),
            &[trap(TrapTrigger::TrappedChest, [0, 0, 0])],
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert_eq!(e.code, "DW0917");
    }

    #[test]
    fn every_trigger_kind_passes_on_its_own_block() {
        for (kind, block) in [
            (
                TrapTrigger::TrappedChest,
                "minecraft:trapped_chest[facing=west,type=single,waterlogged=false]",
            ),
            (TrapTrigger::PressurePlate, "minecraft:stone_pressure_plate"),
            (
                TrapTrigger::PressurePlate,
                "minecraft:heavy_weighted_pressure_plate[power=0]",
            ),
            (TrapTrigger::Tripwire, "minecraft:tripwire[attached=true]"),
        ] {
            assert!(
                check_trap_triggers(
                    &crate::compiler::blockstate::interned(world([1, 2, 3], block)),
                    &[trap(kind, [1, 2, 3])],
                    &BTreeMap::new()
                )
                .is_ok(),
                "{block}"
            );
        }
    }

    /// A kind judged against another kind's block is refused: a plate trap on a
    /// tripwire cell would detect a step the tripwire never shows.
    #[test]
    fn a_plate_trap_on_a_tripwire_is_dw0917() {
        let e = check_trap_triggers(
            &crate::compiler::blockstate::interned(world([1, 2, 3], "minecraft:tripwire")),
            &[trap(TrapTrigger::PressurePlate, [1, 2, 3])],
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert_eq!(e.code, "DW0917");
    }
}
