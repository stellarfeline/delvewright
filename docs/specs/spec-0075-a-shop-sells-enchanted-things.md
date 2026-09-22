# spec-0075: A shop sells enchanted things

- **Status**: Accepted
- **Ground**: three findings from one playtest of a campaign under
  construction and the content round that followed it, each re-read against
  the engine at `fa95d173` (`origin/main`): the prefab palette allowlist
  (`crates/delvec/src/admit/allowlist.rs`), the trap trigger emission
  (`compiler::emit`, `trap_setup` / `trap_fire_tick`), and the item-stack
  surfaces of the quests stage (`Verb::GiveItem`, `LootItem`, `EquipItem`).
- **What it is for**: a merchant can sell an enchanted sword and an enchanted
  book, an anvil can stand beside him for the party to use, and a false chest
  the story names is a chest the party can see and open.
- **Research**: vanilla has two item components for enchantments. The
  Minecraft Wiki page *Data component format* (read for this spec) calls
  `minecraft:enchantments` an item's active enchantments and
  `minecraft:stored_enchantments` "inactive enchantments, such as with
  enchanted books", and says the stored ones are the ones an anvil adds to an
  enchantable item. **Cited.** Both component types are in the pinned 1.21.11
  `data_component_type` registry
  (`tools/spike-area-effect-arrow/registries-1.21.11.json`). **Measured.** That
  the game picks the component by whether the item is an enchanted book
  (`EnchantmentHelper.getComponentType`) is from memory of the game's code and
  was not re-read for this spec: **unsupported**, and it decides nothing beyond
  what the wiki sentence already does. The rules built on it are **authored**.
  The PackTest in §4.6 witnesses that each component is stored as written; it
  does not witness an anvil.
- **Numbers**: one spec (this one); one DW code, `DW0917`; the campaign format
  moves one minor step over main's, because the `delvewright-dsl` crate's
  source changes (ADR-0024).
- **Non-goals**: enchantments on a class kit item (`KitItem` reserves the field
  for a later milestone, spec-0001) and on a mob's `drops[]` quest-token form
  (`ItemDrop`); a price field (spec-0032 rules there is none); an anvil the
  engine repairs after use.

## 1. The findings

1. **A piece could not carry an anvil or a trapped chest.** The default palette
   allowlist (`DW0730`) refused `minecraft:anvil`, `chipped_anvil`,
   `damaged_anvil` and `minecraft:trapped_chest`. The list's own rationale
   flags redstone contraption parts, tnt and note blocks; it already admits the
   trap trigger family (`_pressure_plate`, `_button`) and job-site furniture a
   player uses (grindstone, smithing table). Neither refused block is in a
   class the rationale names: a trapped chest is the visible trigger of the
   engine's own `trapped-chest` trap, and an anvil's fall is the gravity gate's
   business (`compiler::assembled` classes all three stages as gravity blocks).
   The omission was the defect.
2. **A trapped-chest trap shipped over empty air, and nothing refused it.**
   A trap's `trigger` names hardware the piece places. The compiler only
   detects it — a position test on the cell for a plate or a tripwire, an
   invisible `minecraft:interaction` hitbox for a trapped chest — and neither
   needs the block. So a trap whose cell held air compiled, passed its
   completability proofs and fired on a cell that showed the player nothing.
   The gap let a broken trap ship; it could never turn a proof red.
3. **A shop could not sell an enchanted item, and nothing could hand over an
   enchanted book.** `give-item` took `{item, count, name?, carrier?}`; a
   `loot` stack and an equipped piece already carried `enchantments`. By the
   object-class rule the field belongs to the item stack, so `give-item` takes
   the stack description the other surfaces take, not a bespoke field.

## 2. The surface

`give-item` gains `enchantments`: an object of enchantment id → level, the
field a `loot` stack carries, absent by default. Nothing else in the effect
vocabulary moves.

## 3. The rules

1. **One component rule.** `delvewright_dsl::enchantment_component(item)` is
   vanilla's: `minecraft:stored_enchantments` for `minecraft:enchanted_book`,
   `minecraft:enchantments` for every other item. Every emitter that writes an
   enchanted stack asks it: a `give-item`, a `loot` fill, an equipped piece.
   The author writes one field, `enchantments`, on a book as on a sword; the
   component is a derivation, never typed.
2. **One renderer.** A `give-item` stack is rendered by the renderer a
   container fill uses, so a named, unenchanted give emits the pre-existing line
   byte for byte and a given stack and a container stack of one item agree.
3. **One check.** `DW0433`/`DW0434` judge a `give-item`'s `enchantments` at
   every effect root and any nesting depth, on the same terms as a `loot`
   stack's.
4. **A trap's trigger block stands at its cell (`DW0917`).** Build tier. The
   cell of every trap's `at` anchor in the assembled world (the edited one when
   a stage-7 script exists) holds the block its `trigger` names: any
   `*_pressure_plate`, `minecraft:tripwire`, `minecraft:trapped_chest`. A trap
   inside a gate region is refused too, since the gate clears it.
5. **The allowlist admits the four blocks** of finding 1 and keeps refusing the
   contraption parts its rationale names.

## 4. Acceptance criteria

Machine-checkable; each names its instrument. `delvec` is this tree's
`target/debug/delvec`.

1. **The allowlist.** `crates/delvec/tests/admit_audit.rs::an_anvil_and_a_trapped_chest_pass_the_default_allowlist`
   audits a piece carrying the three anvil stages and a trapped chest against
   the default allowlist and passes, and the same test refuses
   `minecraft:dispenser` with `DW0730`. Red before the change (four `DW0730`
   findings), green after.
2. **The trap trigger.** `crates/delvec/src/compiler/trap_trigger.rs` unit tests
   refuse a trapped-chest trap over air, a trapped-chest trap over a plain
   chest, and a plate trap over a tripwire with `DW0917`, and pass each trigger
   kind over its own block.
   `crates/delvec/tests/v06_traps.rs::a_trapped_chest_trap_with_no_chest_is_dw0917`
   builds the same trap over the piece with and without the chest: without, the
   build exits with `DW0917` naming the trap, its cell and
   `minecraft:trapped_chest`; with, it builds. With the check removed, the first
   half fails ("a trapped-chest trap over empty air built").
3. **The gallery binds the trap and refuses its absence.** The gallery's
   `trap/chest` stands on `anchor/strongbox`, whose cell holds the trapped chest
   `prefabs/gallery-generator` places; the probe
   `gallery/probes/a-chest-that-is-not-there` hangs it on open floor and
   `tools/ci/check-gallery-coverage.py` reports it refused with `DW0917`.
4. **The surface.** `crates/dsl/tests/give_item_enchantments.rs`: a
   `give-item` with `enchantments` on a sword and on an enchanted book validates
   clean (before the change the key was `DW0100`); the map reaches
   `Verb::GiveItem` in id order and an unenchanted give serialises without the
   key; an unknown id is `DW0433` and a level 0 is `DW0434` at the verb's own
   pointer; both reach a shop offer and a `sequence` step inside one.
5. **The emission.** `crates/delvec/tests/v10_economy.rs::a_shop_sells_an_enchanted_sword_and_an_enchanted_book`
   reads `shop_pick_0_0` and finds
   `give @s minecraft:iron_sword[enchantments={"minecraft:sharpness":2}] 1` and
   `give @s minecraft:enchanted_book[stored_enchantments={"minecraft:mending":1}] 1`;
   a named, unenchanted give in the same shop emits no enchantment component.
   `emit::build` validates every line against `CommandTree::v1_21_11`.
   `compiler::emit::loot_emit_tests::an_enchanted_book_stores_its_enchantments_on_every_surface`
   shows a `loot` book and an equipped book use the stored component.
6. **The live witness.** For every shop offer that hands over an enchanted
   stack, the compiler generates
   `packtest-datapack/data/<ns>/test/shop_enchanted_stack_<i>_<j>.mcfunction`:
   it empties a pinned dummy, drives the offer's gate open, probes for each
   stack before the purchase (must read 0) and after it (must read 1) with an
   item predicate naming exactly the enchantments in the item's component. The
   gallery's `shop/counter` offer 2 sells an iron sword with Sharpness II and an
   enchanted book storing Mending I, and the gallery's PackTest suite
   (`validation/packtest-run.sh`) passes it on the pinned 1.21.11 server.
7. **Byte identity.** A campaign that hands over no enchanted stack, hangs every
   trap on its trigger block and puts no enchanted book in a container emits
   what it emitted before, apart from the stamped format number:
   `tools/ci/gallery-baseline.py` attributes every moved gallery path.
