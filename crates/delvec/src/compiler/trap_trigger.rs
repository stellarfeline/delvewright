//! **A trap's trigger is a block the party sees and springs, so it has to be
//! there** (`DW0917`).
//!
//! A trap's `trigger` names a kind of hardware — a pressure plate, a tripwire,
//! a trapped chest — and the prefab places it, exactly as it places a `loot`
//! container. The compiler never places a trigger. What
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
/// names. Returns the number of trigger cells it read off the world. `anchors` is only read for the remedy: the anchors of this world
/// whose cell already holds a matching block.
pub fn check_trap_triggers(
    blocks: &crate::compiler::blockstate::BlockMap,
    traps: &[TrapPlan],
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Result<usize, Failure> {
    let at =
        |c: &[i32; 3]| -> &str { blocks.get(c).map(|s| s.as_str()).unwrap_or("minecraft:air") };
    let mut bad: Vec<String> = Vec::new();
    let mut judged = 0usize;
    for t in traps {
        judged += 1;
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
        return Ok(judged);
    }
    Err(Failure {
        code: DW_TRAP_TRIGGER_MISSING,
        message: format!(
            "{} trap(s) have no trigger block at their trigger cell.\n{}\n\
             A trap's trigger is hardware the PREFAB places, exactly as a `loot` container \
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
) -> Result<usize, Failure> {
    let at =
        |c: &[i32; 3]| -> &str { blocks.get(c).map(|s| s.as_str()).unwrap_or("minecraft:air") };
    let mut bad: Vec<String> = Vec::new();
    let mut judged = 0usize;
    for t in steps {
        judged += 1;
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
        return Ok(judged);
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

/// What the trap checks bound to on one build: the denominator is the traps
/// the campaign declares, every judged count comes from the check that did the
/// judging, and a declaration of zero is legitimate and prints its zeroes.
///
/// The world half — `DW0917` over the trigger cells, `DW0352` over the
/// trigger and disarm cells — is [`TrapBinding`]; the runtime half — the
/// `DW0811` trap-fire claim — is [`TrapFireBinding`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrapBinding {
    /// Traps the campaign declares.
    pub traps: usize,
    /// Of those, per trigger kind (`pressure-plate` / `tripwire` /
    /// `trapped-chest`), every kind listed whether or not any trap uses it.
    pub by_trigger: BTreeMap<&'static str, usize>,
    /// Payload effects the traps declare, per verb tag (top-level effects).
    pub payload_effects: BTreeMap<&'static str, usize>,
    /// Trigger cells [`check_trap_triggers`] read off the assembled world.
    pub cells_judged: usize,
    /// `step` triggers the campaign declares.
    pub steps: usize,
    /// `step` trigger cells [`check_step_triggers`] read off the world.
    pub steps_judged: usize,
    /// Traps that declare a disarm affordance.
    pub disarms: usize,
    /// Trigger and disarm cells `DW0352` guards from a world-edits write.
    pub hardware_cells: usize,
    /// World-edits batches `DW0352` was run over.
    pub edit_batches: usize,
    /// Cells those batches wrote, each compared with the hardware cells.
    pub edit_writes: usize,
}

impl TrapBinding {
    /// The declared half, counted off the plan's own objects.
    #[must_use]
    pub fn declared(
        traps: &[TrapPlan],
        steps: usize,
        hardware_cells: usize,
        edit_batches: usize,
        edit_writes: usize,
    ) -> Self {
        let mut by_trigger: BTreeMap<&'static str, usize> = [
            delvewright_dsl::TrapTrigger::PressurePlate,
            delvewright_dsl::TrapTrigger::Tripwire,
            delvewright_dsl::TrapTrigger::TrappedChest,
        ]
        .iter()
        .map(|k| (k.kind(), 0))
        .collect();
        let mut payload_effects: BTreeMap<&'static str, usize> = BTreeMap::new();
        for t in traps {
            *by_trigger.entry(t.trigger.kind()).or_default() += 1;
            for e in &t.payload_effects {
                *payload_effects.entry(e.verb.tag()).or_default() += 1;
            }
        }
        TrapBinding {
            traps: traps.len(),
            by_trigger,
            payload_effects,
            steps,
            disarms: traps.iter().filter(|t| t.disarm.is_some()).count(),
            hardware_cells,
            edit_batches,
            edit_writes,
            ..TrapBinding::default()
        }
    }

    /// Record what the two `DW0917` checks judged.
    pub fn judged(&mut self, cells: usize, steps: usize) {
        self.cells_judged = cells;
        self.steps_judged = steps;
    }

    /// The number a staging gate reads: trigger cells judged.
    #[must_use]
    pub fn examined(&self) -> usize {
        self.cells_judged
    }

    /// One line, for stderr, printed before the verdict.
    #[must_use]
    pub fn line(&self) -> String {
        let kinds: Vec<String> = ["pressure-plate", "tripwire", "trapped-chest"]
            .iter()
            .map(|k| format!("{} {k}", self.by_trigger.get(k).copied().unwrap_or(0)))
            .collect();
        let payload = if self.payload_effects.is_empty() {
            "no payload effect".to_string()
        } else {
            let v: Vec<String> = self
                .payload_effects
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect();
            format!("payload effects {}", v.join(", "))
        };
        format!(
            "trap binding: {t} trap(s) declared ({kinds}; {payload}); {j} of {t} trigger cell(s) \
             judged against the assembled world (DW0917), {sj} of {s} `step` trigger cell(s); \
             {d} disarm(s) declared; {h} trigger/disarm cell(s) guarded against {w} written \
             cell(s) in {b} world-edits batch(es) (DW0352).",
            t = self.traps,
            kinds = kinds.join(", "),
            j = self.cells_judged,
            s = self.steps,
            sj = self.steps_judged,
            d = self.disarms,
            h = self.hardware_cells,
            w = self.edit_writes,
            b = self.edit_batches,
        )
    }

    /// A trap the campaign declares whose cell the check never read: the check
    /// was handed a smaller list than the plan holds, so its pass said nothing
    /// about the rest.
    #[must_use]
    pub fn finding(&self) -> Option<Failure> {
        if self.cells_judged == self.traps {
            return None;
        }
        Some(Failure {
            code: DW_TRAP_TRIGGER_MISSING,
            message: format!(
                "internal invariant violation: the trap-trigger check read {} trigger cell(s) \
                 for {} declared trap(s), so it was judging a smaller world than it claims to \
                 cover and its pass would have meant nothing. Something filtered the list the \
                 check is handed. This is a compiler bug; stop and escalate.",
                self.cells_judged, self.traps
            ),
        })
    }

    /// The binding ledger (`validation/trap-binding.json`), written by every
    /// campaign: a campaign with no trap ships zeroes rather than nothing.
    #[must_use]
    pub fn ledger(&self) -> serde_json::Value {
        serde_json::json!({
            "examined": self.examined(),
            "traps": self.traps,
            "by_trigger": self.by_trigger,
            "payload_effects": self.payload_effects,
            "steps": self.steps,
            "steps_judged": self.steps_judged,
            "disarms": self.disarms,
            "hardware_cells": self.hardware_cells,
            "edit_batches": self.edit_batches,
            "edit_writes": self.edit_writes,
        })
    }
}

/// What the `DW0811` trap-fire claim bound to on one build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrapFireBinding {
    /// Traps the campaign declares.
    pub traps: usize,
    /// Fire PackTests found in the emitted suite, one expected per trap.
    pub tests_emitted: usize,
    /// `trap_fire_<id>` bodies the claim judged.
    pub bodies_judged: usize,
    /// Of those, the ones the suite drives.
    pub bodies_driven: usize,
}

impl TrapFireBinding {
    /// Count the fire PackTests off the emitted tree, and the claim's own
    /// judgement of the `trap_fire_` bodies.
    #[must_use]
    pub fn measure(
        ns: &str,
        traps: &[TrapPlan],
        out: &BTreeMap<String, Vec<u8>>,
        claim: &crate::compiler::watch::Claim,
    ) -> Self {
        let tests_emitted = traps
            .iter()
            .filter(|t| {
                out.contains_key(&format!(
                    "packtest-datapack/data/{ns}/test/v06_trap_fire_{}.mcfunction",
                    t.safe
                ))
            })
            .count();
        let (b, _) = crate::compiler::watch::check_claims(ns, out, std::slice::from_ref(claim));
        TrapFireBinding {
            traps: traps.len(),
            tests_emitted,
            bodies_judged: b.bodies_judged,
            bodies_driven: b.bodies_watched,
        }
    }

    /// One line, for stderr, printed before the verdict.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "trap-fire binding: {} fire PackTest(s) emitted for {} declared trap(s); the \
             trap-fire claim judged {} `trap_fire_` body/bodies, {} driven by the suite (DW0811).",
            self.tests_emitted, self.traps, self.bodies_judged, self.bodies_driven
        )
    }

    /// A declared trap with no fire PackTest in the emitted suite, or a body
    /// the claim never saw: the claim was judging less than the plan declares.
    #[must_use]
    pub fn finding(&self) -> Option<String> {
        if self.tests_emitted == self.traps && self.bodies_judged == self.traps {
            return None;
        }
        Some(format!(
            "internal invariant violation: the trap-fire claim found {} fire PackTest(s) and \
             judged {} `trap_fire_` body/bodies for {} declared trap(s), so it was judging a \
             smaller suite than the campaign declares and its pass would have meant nothing. \
             Something dropped a trap from the emitter's loop or renamed its body. This is a \
             compiler bug; stop and escalate.",
            self.tests_emitted, self.bodies_judged, self.traps
        ))
    }

    /// The binding ledger (`validation/trap-fire.json`), written by every
    /// campaign.
    #[must_use]
    pub fn ledger(&self) -> serde_json::Value {
        serde_json::json!({
            "examined": self.bodies_judged,
            "traps": self.traps,
            "tests_emitted": self.tests_emitted,
            "bodies_judged": self.bodies_judged,
            "bodies_driven": self.bodies_driven,
        })
    }
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

    fn three_traps() -> Vec<TrapPlan> {
        let mut plate = trap(TrapTrigger::PressurePlate, [1, 1, 1]);
        plate.payload_effects = vec![
            delvewright_dsl::Verb::DamagePlayers {
                amount: 2,
                damage_type: None,
            }
            .into(),
        ];
        plate.disarm = Some(crate::compiler::plan::TrapDisarmPlan {
            via_anchor: "anchor/lever".to_string(),
            via_cell: [2, 1, 1],
            sets_flag: "flag/off".to_string(),
        });
        let wire = trap(TrapTrigger::Tripwire, [1, 1, 2]);
        let chest = trap(TrapTrigger::TrappedChest, [1, 1, 3]);
        vec![plate, wire, chest]
    }

    /// The line states every count from the objects, with its denominator, for
    /// a fixture holding each trigger kind.
    #[test]
    fn the_binding_states_each_trigger_kind_and_its_denominators() {
        let traps = three_traps();
        let mut b = TrapBinding::declared(&traps, 2, 4, 3, 17);
        let mut blocks = BTreeMap::new();
        blocks.insert([1, 1, 1], "minecraft:oak_pressure_plate".to_string());
        blocks.insert([1, 1, 2], "minecraft:tripwire".to_string());
        blocks.insert([1, 1, 3], "minecraft:trapped_chest".to_string());
        let judged = check_trap_triggers(
            &crate::compiler::blockstate::interned(blocks),
            &traps,
            &BTreeMap::new(),
        )
        .unwrap();
        b.judged(judged, 2);
        assert_eq!(b.traps, 3);
        assert_eq!(b.by_trigger["pressure-plate"], 1);
        assert_eq!(b.by_trigger["tripwire"], 1);
        assert_eq!(b.by_trigger["trapped-chest"], 1);
        assert_eq!(b.payload_effects["damage-players"], 1);
        assert_eq!(b.disarms, 1);
        assert_eq!(b.examined(), 3);
        assert!(b.finding().is_none());
        let line = b.line();
        assert!(
            line.contains("3 trap(s) declared (1 pressure-plate, 1 tripwire, 1 trapped-chest;"),
            "{line}"
        );
        assert!(line.contains("payload effects 1 damage-players"), "{line}");
        assert!(line.contains("3 of 3 trigger cell(s) judged"), "{line}");
        assert!(line.contains("2 of 2 `step` trigger cell(s)"), "{line}");
        assert!(line.contains("1 disarm(s) declared"), "{line}");
        assert!(
            line.contains("4 trigger/disarm cell(s) guarded against 17 written cell(s) in 3"),
            "{line}"
        );
        assert_eq!(b.ledger()["examined"], 3);
    }

    /// A campaign that declares no trap states its zeroes and passes.
    #[test]
    fn a_campaign_with_no_trap_states_zero_and_passes() {
        let mut b = TrapBinding::declared(&[], 0, 0, 0, 0);
        b.judged(0, 0);
        assert!(b.finding().is_none());
        assert!(
            b.line().starts_with(
                "trap binding: 0 trap(s) declared (0 pressure-plate, 0 tripwire, 0 trapped-chest; \
                 no payload effect); 0 of 0 trigger cell(s)"
            ),
            "{}",
            b.line()
        );
    }

    /// A trap declared whose cell the check was never handed is refused: the
    /// check judged a smaller list than the plan holds.
    #[test]
    fn a_declared_trap_whose_cell_is_never_judged_is_refused() {
        let traps = three_traps();
        let handed = &traps[..2];
        let mut blocks = BTreeMap::new();
        blocks.insert([1, 1, 1], "minecraft:oak_pressure_plate".to_string());
        blocks.insert([1, 1, 2], "minecraft:tripwire".to_string());
        let judged = check_trap_triggers(
            &crate::compiler::blockstate::interned(blocks),
            handed,
            &BTreeMap::new(),
        )
        .expect("the two handed traps hold their hardware");
        let mut b = TrapBinding::declared(&traps, 0, 0, 0, 0);
        b.judged(judged, 0);
        let f = b.finding().expect("2 judged of 3 declared must red");
        assert_eq!(f.code, "DW0917");
        assert!(
            f.message.contains("read 2 trigger cell(s) for 3"),
            "{}",
            f.message
        );
    }

    /// The fire half counts the PackTests off the emitted tree, and refuses a
    /// declared trap that has none.
    #[test]
    fn the_fire_binding_counts_the_emitted_tests_and_refuses_a_missing_one() {
        let traps = three_traps();
        let claim = crate::compiler::watch::Claim {
            mechanic: "trap-fire",
            families: vec!["trap_fire_".to_string()],
            declared: traps.iter().map(|t| t.safe.clone()).collect(),
        };
        let mut out: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        out.insert(
            "datapack/data/ns/function/trap_fire_false_chest.mcfunction".to_string(),
            b"say x\n".to_vec(),
        );
        out.insert(
            "packtest-datapack/data/ns/test/v06_trap_fire_false_chest.mcfunction".to_string(),
            b"function ns:trap_fire_false_chest\n".to_vec(),
        );
        let one = &traps[..1];
        let b = TrapFireBinding::measure("ns", one, &out, &claim);
        assert_eq!(
            (b.traps, b.tests_emitted, b.bodies_judged, b.bodies_driven),
            (1, 1, 1, 1)
        );
        assert!(b.finding().is_none());
        assert!(
            b.line()
                .contains("1 fire PackTest(s) emitted for 1 declared trap(s)")
        );
        // The same suite against a plan that declares one more trap than it drives.
        let mut two = traps[..1].to_vec();
        let mut extra = trap(TrapTrigger::Tripwire, [9, 9, 9]);
        extra.safe = "second".to_string();
        two.push(extra);
        let b = TrapFireBinding::measure("ns", &two, &out, &claim);
        assert_eq!((b.traps, b.tests_emitted), (2, 1));
        assert!(b.finding().is_some(), "a trap with no fire test must red");
    }
}
