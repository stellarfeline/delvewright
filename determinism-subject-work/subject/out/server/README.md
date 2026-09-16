# server/

Level config for campaign `metrics-gym`. The world is generated on first server boot
from `server.properties` (no region files shipped, spec-0002):

- `level-type=minecraft:flat` + `generator-settings` with an empty layer list and
the `minecraft:the_void` biome ⇒ a void world.
- `level-seed=20260821` pins world generation (ADR-0006); v0 uses no other randomness.
- `gamemode=adventure`, `difficulty=peaceful`, no structures/monsters.
- `view-distance=10` / `simulation-distance=10` (chunks) are pinned here rather
than left to the host: the delve renders and ticks the same everywhere.

The compiler-emitted `#minecraft:load` bootstrap (`datapack/`) places each area's
prefab with `/place template` and summons NPCs; nothing is baked into region
bytes, so byte-identity (ADR-0006) covers the whole `<out>/` tree.
