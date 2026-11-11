# `delvec::compiler::assembled`

The reference page for `crates/delvec/src/compiler/assembled.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|
| `DW0313` | A placed gravity block (`sand`/`gravel`/`concrete_powder`/anvil/`dragon_egg`) despawns into the void at placement — an unsupported gravity floor over the void world falls out on the first block update, holing the shipped map even off the critical path. The authoritative gravity-settle gate (`compiler::assembled`), not a downstream DW0311/DW0312 side effect. **Analysis-tier: exit 2** — a prefab/generator defect; the message attributes despawned cells+counts per piece and prescribes a non-falling substrate. Blocks that fall but **land on support** are faithfully modelled by the settle pass (no diagnostic): the shipped geometry is exact for every consumer, and the generator's own zero-unsupported invariant catches an *unintended* fall at authoring. Anti-dodge: swapping the floor palette to non-falling blocks to silence this is explicitly rejected — gravity floors are a first-class content need; add the substrate. |

**A block a shape update turns** (`assembled::turn_unkept`, over `delvewright_dsl::support::turned_under`). Wherever the world is settled — the assembly, and every world-edits batch — a `dirt_path` or a `farmland` under a solid block (`isSolid()`, the `s` flag of `support-bases-1.21.11.tsv`) that is not a fence gate is dirt, as the pinned server keeps it: `DirtPathBlock.canSurvive` (and `FarmBlock`'s) holds only while the block above is not solid or is a fence gate, and the shape update every structure and `setblock` write takes schedules the tick that turns it (`FarmBlock.turnToDirt`), whichever of the two was written first (1.21.11 server, Mojang mappings). Measured on the pinned server: four `dirt_path` cells a farm piece laid under hay stooks were dirt in the save, where the written world held paths (`DW0955`, `tools/ci/check-written-world.py`).
