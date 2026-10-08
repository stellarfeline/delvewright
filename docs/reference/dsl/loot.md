# `delvewright_dsl::loot`

The reference page for `crates/dsl/src/loot.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW043x — geometry & container proofs (stair orientation; spec-0021 loot; `collect` container adoption)

Two unrelated families sharing a number block: proofs that a *block* is the
block the content meant, rather than proofs about quests or timelines.

| Code | Meaning |
|------|---------|
| `DW0435` | Two **positional container fills** claim one anchor: two `loot` entries, or a `loot` entry and a `collect`'s adopted `container`, or two adopted collects. Validation-tier (exit 1). Slots are assigned positionally from `container.0`, so the later fill overwrites the earlier one slot-for-slot and the loser's items never reach the player — and for two collects it is worse: whichever activates second replaces the first objective's items with its own. Prescription: give each fill its own container anchor (prefabs may expose several), or fold the items into one — never rely on declaration order to combine them. |
| `DW0436` | A **single-slot fill**'s `count` exceeds the item's `minecraft:max_stack_size` in the pinned 1.21.11 registry. Validation-tier (exit 1). Covers every DSL surface that compiles to `item replace … container.<n> with <item> <count>`: a `loot[]` stack, a `collect` objective's prop chest, and a trap's `dispense` payload. The command fails **SILENTLY** above the cap — the slot ships empty, the server logs nothing — which is the same silent-failure class `DW0431` exists for: `minecraft:rabbit_stew` (cap 1) declared `count: 2` puts nothing in the chest. The cap is Mojang's own data, vendored per MC pin as `crates/delvec/data/item-stack-sizes-1.21.11.json` (regenerate with `tools/maintenance/extract-item-stack-sizes.py`; a test pins its key set equal to the item registry's) — never a hand-maintained table. 1.21.11 uses exactly three caps: 1, 16, 64. Skipped when the item id is unknown, since that is already `DW0143`. Prescription: lower the count, or add more entries/containers. Do NOT rely on the game splitting the stack — `give` does, `item replace` does not. |
