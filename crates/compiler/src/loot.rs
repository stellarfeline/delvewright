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
use crate::failure::Failure;
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

/// **A container this campaign's assembled world really has, at an anchor a
/// campaign document can name.**
///
/// Both container proofs told an author to *"point it at an anchor whose cell
/// already has one"* and neither said which anchors those are — a remedy phrased
/// as a search over a prefab library the author does not own. The compiler has
/// the answer in its hand at the moment it refuses: it is holding the assembled
/// world and the whole anchor table, which is exactly what the question is about.
///
/// Measured over the library these refusals are met in: of 36 pieces, 5 contain a
/// container blockstate at all and 3 declare an anchor whose NAME says container;
/// the intersection is one piece. So "find an anchor with a container" is not a
/// small search, and the two obvious names (`anchor/chest`, on two different
/// pieces) are both anchors of pieces that carry no container anywhere. An author
/// following the old sentence had no way to discover that except by trying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerAnchor {
    /// The area whose placed piece provides the anchor.
    pub area: String,
    /// The anchor name a campaign document would write.
    pub name: String,
    /// The container actually standing at its cell.
    pub block: String,
    /// How many slots that container has.
    pub slots: usize,
    /// What already fills it, if anything — a `loot` id or a `collect`
    /// objective id. A claimed container is not an available remedy: two fills
    /// of one container is `DW0435`, so offering it would send the author from
    /// one refusal straight into another.
    pub claimed_by: Option<String>,
}

/// Every anchor of the assembled world whose cell holds a container, with what
/// already claims it.
///
/// Takes the resolved-anchor map rather than the [`crate::plan::AnchorTable`]
/// itself, which is the shape every other consumer of resolved anchors in this
/// workspace takes and what `plan.anchors` derefs to. Iteration order is that
/// map's, a `BTreeMap` over `(area, name)`, so the offered list is deterministic
/// (ADR-0006) and reads in area order.
pub fn container_anchors(
    blocks: &BTreeMap<[i32; 3], String>,
    anchors: &BTreeMap<(String, String), crate::plan::ResolvedAnchor>,
    loot: &[LootPlan],
    fills: &[CollectFillPlan],
) -> Vec<ContainerAnchor> {
    // Claims are keyed by anchor NAME, because that is the scope `DW0435`
    // refuses in: it walks the campaign's declarations, which carry a name and
    // no area.
    let mut claimed: BTreeMap<&str, String> = BTreeMap::new();
    for l in loot {
        claimed
            .entry(l.anchor.as_str())
            .or_insert_with(|| format!("loot `{}`", l.id));
    }
    for f in fills {
        claimed
            .entry(f.anchor.as_str())
            .or_insert_with(|| format!("collect `{}`", f.objective_id));
    }
    let mut out = Vec::new();
    for ((area, name), resolved) in anchors {
        let cell = match resolved {
            crate::plan::ResolvedAnchor::Point { pos, .. } => *pos,
            // A gate region is a wall the compiler fills and clears, never
            // furniture — and `crate::assembled` clears the region outright, so
            // its cell holds air by construction. Read anyway rather than
            // skipped: the answer comes from the world, not from a belief about
            // which anchor kinds can carry a block.
            crate::plan::ResolvedAnchor::Gate { from, .. } => *from,
        };
        let found = blocks
            .get(&cell)
            .map(String::as_str)
            .unwrap_or("minecraft:air");
        let Some(slots) = container_slots(found) else {
            continue;
        };
        out.push(ContainerAnchor {
            area: area.clone(),
            name: name.clone(),
            block: base_id(found).to_string(),
            slots,
            claimed_by: claimed.get(name.as_str()).cloned(),
        });
    }
    out
}

/// **The half of a container refusal that says what to do about it.**
///
/// One writer for both proofs. `DW0431` and `DW0438` are the same question about
/// the same object class — is there really a container here — reached through two
/// doors, and the remedy is a fact about the campaign's world rather than about
/// the verb that asked. Written twice, the two copies would drift, and the second
/// author to need it would add a third.
///
/// Three states, and each gets a different sentence because each has a different
/// answer:
///
/// * **free containers exist** — the remedy is a campaign edit and the message
///   names every anchor it can be pointed at, with the block and the slot count,
///   so following the sentence cannot land in the overflow arm of the same code;
/// * **containers exist and every one is claimed** — the remedy is still a
///   campaign edit, but not *that* one: a second fill of a claimed container is
///   `DW0435`, so the message names the claims and says the campaign needs
///   another container rather than another reference to this one;
/// * **the world holds none at all** — and this is the state the old sentence
///   was silently wrong about. There is no campaign edit. A container is
///   furniture the piece places, so the answer is a change to which pieces the
///   campaign binds, or to a piece — which is the prefab library, not this
///   campaign. Saying so is a remedy. Sending the author to look for an anchor
///   that does not exist is not.
fn container_remedy(available: &[ContainerAnchor], what: &str) -> String {
    let free: Vec<&ContainerAnchor> = available
        .iter()
        .filter(|c| c.claimed_by.is_none())
        .collect();
    if !free.is_empty() {
        let list = free
            .iter()
            .map(|c| {
                format!(
                    "\n    `{}` in `{}` — a `{}` with {} slots",
                    c.name, c.area, c.block, c.slots
                )
            })
            .collect::<String>();
        return format!(
            "Point {what} at an anchor whose cell already holds a container. This campaign's \
             assembled world has {n} that nothing else is filling:{list}\n  Each is furniture a \
             placed piece put there, so pointing at one is an edit to this campaign and nothing \
             else. Take the slot count into account: a fill wider than the container it names is \
             the same refusal.",
            n = free.len(),
            list = list,
        );
    }
    if !available.is_empty() {
        let list = available
            .iter()
            .map(|c| {
                format!(
                    "\n    `{}` in `{}` — a `{}`, already filled by {}",
                    c.name,
                    c.area,
                    c.block,
                    c.claimed_by.as_deref().unwrap_or("another fill")
                )
            })
            .collect::<String>();
        return format!(
            "Every container this campaign's assembled world has is already being filled, so \
             there is no anchor here to point {what} at:{list}\n  Do NOT point a second fill at \
             one of them. Slots are assigned positionally from `container.0`, so two fills of one \
             container overwrite each other slot-for-slot and that is `DW0435`. What this \
             campaign needs is another container, which is furniture and lives in a piece: bind \
             an area to a piece that carries one, or have a piece export one — the piece is a \
             prefab-library change, not a change you can make from these documents.",
        );
    }
    format!(
        "**No anchor anywhere in this campaign's assembled world holds a container**, so there is \
         nothing to point {what} at and no edit to these campaign documents can create one. A \
         chest, trapped chest or barrel is FURNITURE: a placed piece puts it there, exactly as a \
         piece places a trap's dispenser, and the campaign fills what the piece placed. The two \
         ways forward are both about the piece, and only the first is yours: bind one of this \
         campaign's areas to a piece that already carries a container, or have the piece you are \
         using export one — that second is a change to the prefab library and goes through the \
         piece's own admission, not through this campaign. Do NOT reach for a `set-block` effect \
         to place the container at runtime, and do NOT hand-patch the `.nbt`."
    )
}

/// Build-tier proof: every `loot` anchor resolves to a cell that really holds a
/// container in the assembled world, and no fill overflows that container.
pub fn check_loot_containers(
    blocks: &BTreeMap<[i32; 3], String>,
    loot: &[LootPlan],
    available: &[ContainerAnchor],
) -> Result<(), Failure> {
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
    Err(Failure {
        code: DW_LOOT_NOT_A_CONTAINER,
        message: format!(
            "{} `loot` declaration(s) do not resolve to a fillable container.\n{}\n\
             A `loot` entry fills a container the PREFAB placed — it never places one, exactly \
             as a trap never places its dispenser. `item replace block … container.<n>` against \
             a non-container fails SILENTLY, so this would have shipped as an empty wall where \
             the stores should be. {remedy}",
            bad.len(),
            bad.join("\n"),
            remedy = container_remedy(available, "the `loot` entry's `anchor`"),
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
    available: &[ContainerAnchor],
) -> Result<(), Failure> {
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
    Err(Failure {
        code: DW_COLLECT_NOT_A_CONTAINER,
        message: format!(
            "{} `collect` objective(s) adopt a container that is not there.\n{}\n\
             A `collect` with a `container` fills furniture the PREFAB placed and never places \
             one itself — that is the whole reason the field exists, so a quest item can live in \
             the barrel the player has been walking past instead of in a chest conjured out of \
             the air beside it. `item replace block … container.<n>` against a non-container \
             fails SILENTLY, so this would have shipped as an uncompletable objective with \
             nothing anywhere to pick up. {remedy} Dropping the `container` field to make this \
             go away is NOT the fix — that silently returns the delve to a floating compiler \
             chest, which is the defect the field was added to remove.",
            bad.len(),
            bad.join("\n"),
            remedy = container_remedy(available, "`container`"),
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
        assert!(check_loot_containers(&b, &[mk("stores", [1, 2, 3], 5)], &[]).is_ok());
    }

    #[test]
    fn a_non_container_cell_is_dw0431_naming_the_block_it_found() {
        let mut b = BTreeMap::new();
        b.insert([1, 2, 3], "minecraft:stone_bricks".to_string());
        let e = check_loot_containers(&b, &[mk("stores", [1, 2, 3], 1)], &[]).unwrap_err();
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
        let e = check_loot_containers(&BTreeMap::new(), &[mk("stores", [0, 0, 0], 1)], &[])
            .unwrap_err();
        assert_eq!(e.code, "DW0431");
    }

    #[test]
    fn overflowing_the_container_is_dw0431() {
        let mut b = BTreeMap::new();
        b.insert([1, 2, 3], "minecraft:chest".to_string());
        let e = check_loot_containers(&b, &[mk("stores", [1, 2, 3], 28)], &[]).unwrap_err();
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
        assert!(check_collect_containers(&b, &[fill([4, 5, 6], 9)], &[]).is_ok());
    }

    #[test]
    fn adopting_a_cell_that_holds_no_container_is_dw0438() {
        let mut b = BTreeMap::new();
        b.insert([4, 5, 6], "minecraft:oak_planks".to_string());
        let e = check_collect_containers(&b, &[fill([4, 5, 6], 1)], &[]).unwrap_err();
        assert_eq!(e.code, "DW0438");
        assert!(e.message.contains("minecraft:oak_planks"), "{}", e.message);
        assert!(e.message.contains("obj/take-cheese"), "{}", e.message);
        assert!(e.message.contains("[4, 5, 6]"), "{}", e.message);
    }

    /// Adopting thin air is the same defect — the anchor resolved, the furniture
    /// was never authored.
    #[test]
    fn adopting_an_empty_cell_is_also_dw0438() {
        let e = check_collect_containers(&BTreeMap::new(), &[fill([0, 0, 0], 1)], &[]).unwrap_err();
        assert_eq!(e.code, "DW0438");
    }

    #[test]
    fn padding_past_the_containers_slots_is_dw0438() {
        let mut b = BTreeMap::new();
        b.insert([4, 5, 6], "minecraft:barrel".to_string());
        let e = check_collect_containers(&b, &[fill([4, 5, 6], 28)], &[]).unwrap_err();
        assert_eq!(e.code, "DW0438");
        assert!(e.message.contains("27 slots"), "{}", e.message);
    }

    /// A campaign whose collects keep the compiler-placed chest declares no fills
    /// at all, so the proof is vacuously green — it can never fire on pre-0.8
    /// content.
    #[test]
    fn no_adoptions_is_vacuously_ok() {
        assert!(check_collect_containers(&BTreeMap::new(), &[], &[]).is_ok());
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

    // --- what the refusal SAYS to do about it ------------------------------
    //
    // A container proof that is right about the defect and wrong about the
    // remedy is the shape CLAUDE.md names: a gate that names a remedy owes a
    // check that the remedy is reachable. The three arms below are the three
    // answers the question has, and the middle one is the one an author cannot
    // reach on their own.

    fn anchor(name: &str, claimed: Option<&str>) -> ContainerAnchor {
        ContainerAnchor {
            area: "area/hall".to_string(),
            name: name.to_string(),
            block: "minecraft:barrel".to_string(),
            slots: 27,
            claimed_by: claimed.map(str::to_string),
        }
    }

    /// A free container is offered **by name**, with the block and the slots,
    /// because "point it at an anchor whose cell already has one" is a search
    /// the compiler had already done.
    #[test]
    fn a_free_container_is_named_in_the_refusal() {
        let e = check_collect_containers(
            &BTreeMap::new(),
            &[fill([0, 0, 0], 1)],
            &[anchor("anchor/case", None)],
        )
        .unwrap_err();
        assert_eq!(e.code, "DW0438");
        assert!(e.message.contains("`anchor/case`"), "{}", e.message);
        assert!(e.message.contains("`area/hall`"), "{}", e.message);
        assert!(e.message.contains("27 slots"), "{}", e.message);
    }

    /// A container something else already fills is **not** offered, and the fact
    /// that it is taken is stated. Offering it would send the author from
    /// `DW0438` straight into `DW0435` — an opt-out the defect itself supplies.
    #[test]
    fn a_claimed_container_is_never_offered_as_the_remedy() {
        let e = check_collect_containers(
            &BTreeMap::new(),
            &[fill([0, 0, 0], 1)],
            &[anchor("anchor/reliquary", Some("loot `loot/stores`"))],
        )
        .unwrap_err();
        assert!(
            !e.message.contains("Point `container` at an anchor"),
            "a taken container must not be offered as somewhere to point: {}",
            e.message
        );
        assert!(e.message.contains("DW0435"), "{}", e.message);
        assert!(e.message.contains("loot `loot/stores`"), "{}", e.message);
    }

    /// **The state the old sentence was wrong about.** With no container
    /// anywhere, "point it at an anchor whose cell already has one" describes an
    /// anchor that does not exist, and the author cannot make one from a campaign
    /// document. The message has to say that, and say what IS theirs to do.
    #[test]
    fn with_no_container_anywhere_the_refusal_says_so_and_names_the_piece() {
        for e in [
            check_collect_containers(&BTreeMap::new(), &[fill([0, 0, 0], 1)], &[]).unwrap_err(),
            check_loot_containers(&BTreeMap::new(), &[mk("stores", [0, 0, 0], 1)], &[])
                .unwrap_err(),
        ] {
            assert!(
                e.message
                    .contains("No anchor anywhere in this campaign's assembled world"),
                "{}",
                e.message
            );
            // The half that is the author's: which piece an area binds.
            assert!(
                e.message
                    .contains("bind one of this campaign's areas to a piece"),
                "{}",
                e.message
            );
            // And the half that is not, said plainly rather than prescribed.
            assert!(e.message.contains("prefab library"), "{}", e.message);
        }
    }

    /// The offered set is computed from the world and the claims, so a rule that
    /// stopped looking at either would show up here rather than in the prose.
    #[test]
    fn container_anchors_reads_the_world_and_the_claims() {
        use crate::plan::ResolvedAnchor;
        let mut blocks = BTreeMap::new();
        blocks.insert([1, 2, 3], "minecraft:barrel[facing=up]".to_string());
        blocks.insert([4, 5, 6], "minecraft:chest".to_string());
        blocks.insert([7, 8, 9], "minecraft:stone".to_string());
        let mut at = BTreeMap::new();
        for (name, pos) in [
            ("anchor/case", [1, 2, 3]),
            ("anchor/reliquary", [4, 5, 6]),
            ("anchor/lectern", [7, 8, 9]),
        ] {
            at.insert(
                ("area/hall".to_string(), name.to_string()),
                ResolvedAnchor::Point { pos, facing: None },
            );
        }
        let mut claimant = mk("stores", [4, 5, 6], 1);
        claimant.anchor = "anchor/reliquary".to_string();
        let got = container_anchors(
            &blocks,
            &at,
            &[claimant],
            &[CollectFillPlan {
                objective_id: "obj/take".to_string(),
                anchor: "anchor/nowhere".to_string(),
                cell: [0, 0, 0],
                slots: 1,
            }],
        );
        // Three anchors, two containers: the stone one is not one.
        assert_eq!(got.len(), 2, "{got:#?}");
        assert_eq!(got[0].name, "anchor/case");
        assert_eq!(got[0].claimed_by, None);
        assert_eq!(got[1].name, "anchor/reliquary");
        assert_eq!(
            got[1].claimed_by.as_deref(),
            Some("loot `stores`"),
            "the loot entry's claim must be read off the plan"
        );
    }
}
