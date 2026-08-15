//! Container-fill proofs over the assembled world (`DW0431` for spec-0021 `loot`,
//! `DW0438` for a DSL v0.8 `collect` that adopts a prefab container).
//!
//! A `loot` declaration fills a container the **prefab already placed** — the
//! same division of labour a trap has with its dispenser. That makes the
//! container a piece of authored hardware, and "is there actually a container
//! at this anchor?" a question only the assembled world can answer.
//!
//! It has to be answered, because the failure is silent. `item replace block …
//! container.<n>` against a cell that is not a container **fails without
//! output** — the same hazard `DW0352` documents for trap dispensers. Nothing
//! in the delve would look wrong at build time; the player would simply find an
//! ordinary wall where the stores were meant to be, which is exactly the class
//! of defect this compiler turns loud.

use std::collections::BTreeMap;

use crate::assembled::base_id;
use crate::nav::NavError;
use crate::plan::{CollectFillPlan, LootPlan};
use delvewright_dsl::DwCode;

const DW_LOOT_NOT_A_CONTAINER: DwCode = DwCode::every_version("DW0431");

/// A `collect` objective adopts a container the assembled world does not have —
/// or one too small for its fill (DSL v0.8).
const DW_COLLECT_NOT_A_CONTAINER: DwCode = DwCode::every_version("DW0438");

/// The container blocks a `loot` fill accepts, with their slot counts.
///
/// Deliberately the *single-block, slot-addressable* inventories a box-garden
/// delve furnishes rooms with. Excluded on purpose: `ender_chest` (per-player,
/// not world state), `shulker_box` (a portable item), and the double chest,
/// which is two block entities and would make `container.<n>` ambiguous about
/// which half it addresses.
pub fn container_slots(name: &str) -> Option<usize> {
    match base_id(name) {
        "minecraft:chest" | "minecraft:trapped_chest" | "minecraft:barrel" => Some(27),
        _ => None,
    }
}

/// Whether `name` is a container a `loot` fill can target.
pub fn is_container(name: &str) -> bool {
    container_slots(name).is_some()
}

/// Build-tier proof: every `loot` anchor resolves to a cell that really holds a
/// container in the assembled world, and no fill overflows that container.
pub fn check_loot_containers(
    blocks: &BTreeMap<[i32; 3], String>,
    loot: &[LootPlan],
) -> Result<(), NavError> {
    let mut bad: Vec<String> = Vec::new();
    for l in loot {
        let c = l.cell;
        let found = blocks
            .get(&c)
            .map(String::as_str)
            .unwrap_or("minecraft:air");
        match container_slots(found) {
            None => bad.push(format!(
                "  loot `{}` -> anchor `{}` at [{}, {}, {}] holds `{}`, not a container",
                l.id,
                l.anchor,
                c[0],
                c[1],
                c[2],
                base_id(found)
            )),
            Some(slots) if l.items.len() > slots => bad.push(format!(
                "  loot `{}` -> anchor `{}` at [{}, {}, {}] declares {} stacks but `{}` has {} slots",
                l.id,
                l.anchor,
                c[0],
                c[1],
                c[2],
                l.items.len(),
                base_id(found),
                slots
            )),
            Some(_) => {}
        }
    }
    if bad.is_empty() {
        return Ok(());
    }
    Err(NavError {
        code: DW_LOOT_NOT_A_CONTAINER,
        message: format!(
            "{} `loot` declaration(s) do not resolve to a fillable container.\n{}\n\
             A `loot` entry fills a container the PREFAB placed — it never places one, exactly \
             as a trap never places its dispenser. `item replace block … container.<n>` against \
             a non-container fails SILENTLY, so this would have shipped as an empty wall where \
             the stores should be. Fix it in the prefab: put a `minecraft:chest`, \
             `minecraft:trapped_chest` or `minecraft:barrel` at the anchor's cell and re-export \
             the `.nbt`, or point the `loot` entry at an anchor whose cell already has one. Do \
             NOT work around it by adding a `set-block` effect to place the container at runtime \
             — the container is furniture, and furniture belongs in the piece.",
            bad.len(),
            bad.join("\n")
        ),
    })
}

/// Build-tier proof: every `collect` objective that **adopts** a container (DSL
/// v0.8) points at a cell that really holds one, with room for the
/// objective's stack plus its padding.
///
/// The same silent failure `DW0431` exists for, reached through the other door.
/// A `collect` that adopts nothing keeps conjuring its own chest and can never
/// fail this — the whole point of adoption is that the container is FURNITURE the
/// prefab authored, so "is it actually there?" is a question about the assembled
/// world, and the answer "no" must be a build error rather than a barrel-shaped
/// hole the player discovers by finding an empty room where the quest item was.
pub fn check_collect_containers(
    blocks: &BTreeMap<[i32; 3], String>,
    fills: &[CollectFillPlan],
) -> Result<(), NavError> {
    let mut bad: Vec<String> = Vec::new();
    for f in fills {
        let c = f.cell;
        let found = blocks
            .get(&c)
            .map(String::as_str)
            .unwrap_or("minecraft:air");
        match container_slots(found) {
            None => bad.push(format!(
                "  collect `{}` -> container anchor `{}` at [{}, {}, {}] holds `{}`, not a container",
                f.objective_id,
                f.anchor,
                c[0],
                c[1],
                c[2],
                base_id(found)
            )),
            Some(slots) if f.slots > slots => bad.push(format!(
                "  collect `{}` -> container anchor `{}` at [{}, {}, {}] fills {} slots but `{}` has {} slots",
                f.objective_id,
                f.anchor,
                c[0],
                c[1],
                c[2],
                f.slots,
                base_id(found),
                slots
            )),
            Some(_) => {}
        }
    }
    if bad.is_empty() {
        return Ok(());
    }
    Err(NavError {
        code: DW_COLLECT_NOT_A_CONTAINER,
        message: format!(
            "{} `collect` objective(s) adopt a container that is not there.\n{}\n\
             A `collect` with a `container` fills furniture the PREFAB placed and never places \
             one itself — that is the whole reason the field exists, so a quest item can live in \
             the barrel the player has been walking past instead of in a chest conjured out of \
             the air beside it. `item replace block … container.<n>` against a non-container \
             fails SILENTLY, so this would have shipped as an uncompletable objective with \
             nothing anywhere to pick up. Fix it in the prefab: put a `minecraft:chest`, \
             `minecraft:trapped_chest` or `minecraft:barrel` at the anchor's cell and re-export \
             the `.nbt`, or point `container` at an anchor whose cell already has one. Dropping \
             the `container` field to make this go away is NOT the fix — that silently returns \
             the delve to a floating compiler chest, which is the defect the field was added to \
             remove.",
            bad.len(),
            bad.join("\n")
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_accepted_containers_have_27_slots() {
        for id in [
            "minecraft:chest",
            "minecraft:trapped_chest",
            "minecraft:barrel",
        ] {
            assert_eq!(container_slots(id), Some(27), "{id}");
        }
    }

    #[test]
    fn blockstate_variants_still_resolve() {
        assert!(is_container("minecraft:barrel[facing=up,open=false]"));
        assert!(is_container(
            "minecraft:chest[facing=north,type=single,waterlogged=false]"
        ));
    }

    fn mk(id: &str, cell: [i32; 3], n: usize) -> LootPlan {
        LootPlan {
            id: id.to_string(),
            anchor: format!("anchor/{id}"),
            cell,
            items: (0..n)
                .map(|_| crate::plan::LootItemPlan {
                    item: "minecraft:bread".to_string(),
                    count: 1,
                    name: None,
                    enchantments: BTreeMap::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_real_container_passes() {
        let mut b = BTreeMap::new();
        b.insert([1, 2, 3], "minecraft:barrel[facing=up]".to_string());
        assert!(check_loot_containers(&b, &[mk("stores", [1, 2, 3], 5)]).is_ok());
    }

    #[test]
    fn a_non_container_cell_is_dw0431_naming_the_block_it_found() {
        let mut b = BTreeMap::new();
        b.insert([1, 2, 3], "minecraft:stone_bricks".to_string());
        let e = check_loot_containers(&b, &[mk("stores", [1, 2, 3], 1)]).unwrap_err();
        assert_eq!(e.code, "DW0431");
        assert!(
            e.message.contains("minecraft:stone_bricks"),
            "{}",
            e.message
        );
        assert!(e.message.contains("[1, 2, 3]"), "{}", e.message);
    }

    /// An anchor over thin air is the same defect, and must not be silent.
    #[test]
    fn an_empty_cell_is_also_dw0431() {
        let e = check_loot_containers(&BTreeMap::new(), &[mk("stores", [0, 0, 0], 1)]).unwrap_err();
        assert_eq!(e.code, "DW0431");
    }

    #[test]
    fn overflowing_the_container_is_dw0431() {
        let mut b = BTreeMap::new();
        b.insert([1, 2, 3], "minecraft:chest".to_string());
        let e = check_loot_containers(&b, &[mk("stores", [1, 2, 3], 28)]).unwrap_err();
        assert_eq!(e.code, "DW0431");
        assert!(e.message.contains("27 slots"), "{}", e.message);
    }

    fn fill(cell: [i32; 3], slots: usize) -> CollectFillPlan {
        CollectFillPlan {
            objective_id: "obj/take-cheese".to_string(),
            anchor: "anchor/beach-barrel".to_string(),
            cell,
            slots,
        }
    }

    #[test]
    fn an_adopted_barrel_that_is_really_there_passes() {
        let mut b = BTreeMap::new();
        b.insert(
            [4, 5, 6],
            "minecraft:barrel[facing=up,open=false]".to_string(),
        );
        assert!(check_collect_containers(&b, &[fill([4, 5, 6], 9)]).is_ok());
    }

    #[test]
    fn adopting_a_cell_that_holds_no_container_is_dw0438() {
        let mut b = BTreeMap::new();
        b.insert([4, 5, 6], "minecraft:oak_planks".to_string());
        let e = check_collect_containers(&b, &[fill([4, 5, 6], 1)]).unwrap_err();
        assert_eq!(e.code, "DW0438");
        assert!(e.message.contains("minecraft:oak_planks"), "{}", e.message);
        assert!(e.message.contains("obj/take-cheese"), "{}", e.message);
        assert!(e.message.contains("[4, 5, 6]"), "{}", e.message);
    }

    /// Adopting thin air is the same defect — the anchor resolved, the furniture
    /// was never authored.
    #[test]
    fn adopting_an_empty_cell_is_also_dw0438() {
        let e = check_collect_containers(&BTreeMap::new(), &[fill([0, 0, 0], 1)]).unwrap_err();
        assert_eq!(e.code, "DW0438");
    }

    #[test]
    fn padding_past_the_containers_slots_is_dw0438() {
        let mut b = BTreeMap::new();
        b.insert([4, 5, 6], "minecraft:barrel".to_string());
        let e = check_collect_containers(&b, &[fill([4, 5, 6], 28)]).unwrap_err();
        assert_eq!(e.code, "DW0438");
        assert!(e.message.contains("27 slots"), "{}", e.message);
    }

    /// A campaign whose collects keep the compiler-placed chest declares no fills
    /// at all, so the proof is vacuously green — it can never fire on pre-0.8
    /// content.
    #[test]
    fn no_adoptions_is_vacuously_ok() {
        assert!(check_collect_containers(&BTreeMap::new(), &[]).is_ok());
    }

    #[test]
    fn non_containers_and_deliberate_exclusions_are_rejected() {
        for id in [
            "minecraft:stone",
            "minecraft:air",
            "minecraft:ender_chest",
            "minecraft:shulker_box",
            "minecraft:furnace",
        ] {
            assert!(!is_container(id), "{id} must not be fillable");
        }
    }
}
