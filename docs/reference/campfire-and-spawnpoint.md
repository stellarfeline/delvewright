# A campfire, and where a respawn puts a body

What the pinned game does to a body at a campfire, and where `/spawnpoint` puts a player who dies. Reader: the engine work that derives a checkpoint's respawn seat (`nav::World::rest_seat`, `docs/reference/compiler.md`, *Respawn seat*). Established once against the pinned jar and treated as constant (CLAUDE.md, *A check guards only what our toolchain owns*); the read site is stated so the reading can be repeated when the pin moves.

## The read site

- The server jar `versions.toml [minecraft].server_jar_url`, sha256 `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726` (the pin), its inner `META-INF/versions/1.21.11/server-1.21.11.jar`.
- The ProGuard mappings piston-meta publishes for the version, `downloads.server_mappings`, sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`, which give every class and member below its Mojang name; no obfuscated name is written here.
- `javap -p -c` from a JDK 21 (21.0.13) over `CampfireBlock`, `Blocks`, `Entity`, `BlockBehaviour`, `BlockBehaviour$BlockStateBase`, `Block`, `ServerPlayer`, `TeleportTransition` and `SetSpawnCommand`, and the jar's own data files `data/minecraft/damage_type/campfire.json` and `data/minecraft/tags/damage_type/is_fire.json`.

## A campfire

### What the campfire's bytecode says (cited)

1. **`CampfireBlock.entityInside`** loads `LIT` from the state; when it is true and the entity is a `LivingEntity`, it calls `hurt(level.damageSources().campfire(), fireDamage)`. When `LIT` is false, or the entity is not living, it does nothing of its own and defers to `BaseEntityBlock`.
2. **`fireDamage`** is the constructor's second argument. `Blocks` builds `campfire` with `(spawnParticles = true, fireDamage = 1)` and `soul_campfire` with `(false, 2)`, lighting 15 and 10 when lit.
3. **Which cells.** `Entity.checkInsideBlocks` walks the cells the entity's box passes through and, for each, asks the block state for its entity-inside shape (`BlockBehaviour.getEntityInsideCollisionShape`). `CampfireBlock` does not override it, and the default returns `Shapes.block()`, the whole cell; `entityInside` runs when the box's sweep meets that cell. So a body is hurt whenever its box enters the campfire's cell — not only on the block's collision top.
4. **The collision top** is `Block.column(16, 0, 7)`: 7/16 of a block, under the 0.6 auto-step. A body walking into the cell steps up onto it and stands with its feet 7/16 into the campfire's cell.
5. **The damage**: `damage_type/campfire` is tagged `is_fire` (so a fire-immune entity ignores it) and `no_knockback`; it scales `when_caused_by_living_non_player`.

### Measured on a running server

A throwaway pinned server, `ghcr.io/stellarfeline/delvewright-base@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de` (`versions.toml [images.base]`), `TYPE=VANILLA VERSION=1.21.11`, a flat world, no host port, `random_tick_speed 0`, a stone floor at y 63. Per trial, one block set at `x 64 200` by `setblock`, and three pigs (`Health:10f`, `movement_speed` base 0) summoned: one at `x.5 64.5 200.5` (it settles on the block's top), one centred in the cell beside it (`x+1.5 64 200.5`), and a control on bare stone six blocks off. Health read by `data get entity` three seconds later, through `tools/lib/rcon.sh`, twice:

| block | pig on top | pig beside | control |
|---|---|---|---|
| `campfire[lit=true]` | 3.0, then 2.0 (feet at y 64.4375) | 10.0 | 10.0 |
| `campfire[lit=false]` | 10.0 (feet at y 64.4375) | 10.0 | 10.0 |
| `soul_campfire[lit=true]` | dead | 10.0 | 10.0 |
| `soul_campfire[lit=false]` | 10.0 | 10.0 | 10.0 |

The lit state of each block was read back with `execute if block … #minecraft:campfires[lit=true]` (passed for the two lit ones, failed for the two unlit).

### What follows for a seat (authored)

- A lit campfire hurts every living body whose box enters its cell: a body standing on it always, a body beside it only when it presses into the cell. A body centred in the next cell is 0.3 blocks from the shared face and is not hurt.
- An unlit campfire hurts nothing. The engine's seat rule still treats every campfire as a block that hurts (`blockshape::hurts_body` reads no state): a seat beside a cold fire is a seat a body can stand on, and the one fact the rule cannot see — whether the story lights the fire later — moves no seat into it.
- A respawn seat is therefore never the campfire's cell nor the cell over it, and is a cell beside it.

## Where `/spawnpoint` puts a body

### What the respawn's bytecode says (cited)

1. **`SetSpawnCommand.setSpawn`** gives each target `setRespawnPosition(new RespawnConfig(RespawnData.of(dimension, pos, yaw, pitch), forced = true), false)`.
2. **`ServerPlayer.findRespawnAndUseSpawnBlock`** reads the recorded block. A bed or a respawn anchor has its own rules. For any other block, a config that is not forced finds nothing; a forced one asks `isPossibleToRespawnInThis` of the block at the position **and** of the block above it, and when both answer yes returns the position `(x + 0.5, y + 0.1, z + 0.5)`. Otherwise it returns empty, and `findRespawnPositionAndUseSpawnBlock` answers with `TeleportTransition.missingRespawnBlock`, the world spawn flagged as a missing respawn block.
3. **`Block.isPossibleToRespawnInThis`** is `!state.isSolid() && !state.liquid()` for every block but a banner, a pressure plate and a sign, which always answer yes. `BlockStateBase.isSolid` is computed once (`calculateSolid`): a forced-solid or forced-non-solid block keeps its flag; otherwise a block is solid when its collision box's mean side is at least `0.7291666` or its height is at least a block. A campfire's box is 1 × 7/16 × 1, mean side 0.8125, so a campfire is solid.

### What follows for the re-seat (authored, from the reading above)

- A `spawnpoint` on a campfire's cell, or on any cell whose block or the block above is solid or a liquid, is discarded at the respawn, and the player comes back at the world spawn. The engine re-seats the respawned player by teleport for exactly this reason (`compiler.md`, the death edge's `cp_seat_<i>`), and the same teleport onto a campfire's cell would put the body in the fire. So the seat is derived beside the fire, and `spawnpoint`, the `dw:cp` mirror and the re-seat all name that cell.
- On an air cell over a floor, vanilla puts the body at `(x + 0.5, y + 0.1, z + 0.5)` and the re-seat teleport to the cell's centre is a no-op the player cannot see. The water case (a liquid cell, so the world spawn) was measured live when the re-seat was built (`compiler.md`, the death edge); the campfire case is read from the bytecode above and not run live.
