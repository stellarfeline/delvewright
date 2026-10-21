# `delvewright_dsl::loot`

The reference page for `crates/dsl/src/loot.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW043x — geometry & container proofs (stair orientation; spec-0021 loot; `collect` container adoption)

Two unrelated families sharing a number block: proofs that a *block* is the
block the content meant, rather than proofs about quests or timelines.

| Code | Meaning |
|------|---------|
| `DW0435` | Two **fills** claim one anchor: two `loot` entries, or a `loot` entry and a `collect`'s adopted `holder`, or two adopted collects. Validation-tier (exit 1). Slots are assigned positionally from `container.0`, so the later fill overwrites the earlier one slot-for-slot and the loser's items never reach the player — and for two collects it is worse: whichever activates second replaces the first objective's items with its own. Prescription: give each fill its own container anchor (prefabs may expose several), or fold the items into one — never rely on declaration order to combine them. |
| `DW0436` | A **single-slot fill**'s `count` exceeds the item's `minecraft:max_stack_size` in the pinned 1.21.11 registry. Validation-tier (exit 1). Covers every DSL surface that compiles to `item replace … container.<n> with <item> <count>`: a `loot[]` stack, a `collect` objective's prop chest, and a trap's `dispense` payload. The command fails **SILENTLY** above the cap — the slot ships empty, the server logs nothing — which is the same silent-failure class `DW0431` exists for: `minecraft:rabbit_stew` (cap 1) declared `count: 2` puts nothing in the chest. The cap is Mojang's own data, vendored per MC pin as `crates/delvec/data/item-stack-sizes-1.21.11.json` (regenerate with `tools/maintenance/extract-item-stack-sizes.py`; a test pins its key set equal to the item registry's) — never a hand-maintained table. 1.21.11 uses exactly three caps: 1, 16, 64. Skipped when the item id is unknown, since that is already `DW0143`. Prescription: lower the count, or add more entries/containers. Do NOT rely on the game splitting the stack — `give` does, `item replace` does not. |

### DW101x — the container class (spec-0105)

| Code | Meaning |
|------|---------|
| `DW1035` | A fill names a slot the holder class does not have, or a stack a slot cannot wear (spec-0111 §6.1). Validation tier, `loot::loot_checks` and `loot::collect_slot_checks`, through one stack check (`check_stack_slot`) a `loot` stack's `slot` and a `collect`'s `slot` both pass: a `slot` outside every family's vocabulary (`container.<n>` under 27, `contents`, `armor.head`, `armor.chest`, `armor.legs`, `armor.feet`, `weapon.mainhand`, `weapon.offhand`); a slot named twice in one fill; a fill mixing named and unnamed stacks; a stack in `armor.chest`, `armor.legs` or `armor.feet` whose `equippable` slot (the pinned item data, `ItemRegistry::equippable`) is not that slot; and a `slot` on a `collect` with no `holder` (the compiler's own chest, or a `dropped_by` item). Build tier, `compiler::loot`, where the holder at the anchor is known, through one fill proof (`fill_fault`) both doors run: a stand fill — a `loot` stack or a `collect` — that names no slot, and a `collect` with `fill_count > 0` adopting a frame or a stand, which have no `container.<n>` slots. Every message prints each family's vocabulary, names the slot and the stack, and says which slots take any stack. |
| `DW1019` | A container declaration that holds nothing. Validation-tier (exit 1), `loot::container_declaration_checks`. Three shapes: a `loot[]` entry whose `items` is empty (a container meant to hold nothing is scenery, declared once by `containers.decoration_notice`, never by an empty fill); a `containers` object that declares nothing; a runtime `set-block` or `fill-region`, at any effect root or nested inside one, whose block is a container kind of `delvewright_dsl::blocks::CONTAINER_KINDS_1_21_11` — it stands empty for ever, because every fill runs at `setup_finish`, before it exists. The last names what a runtime container can be: a `collect`'s own chest, which the compiler places and fills, or a container the piece places, filled by `loot`. |
