# When the server sends an entity to a client

For the agent who maintains the live half of the shot-subject check (`harness/src/shot-subjects.ts`, spec-0110 §7) and anyone who writes a bot assertion about what a client has been sent. It answers four questions: when the pinned server sends an entity to a client; what `view_range` on a display entity changes; what a spectating player's position is during a cutscene; and in what order a cutscene's packets reach the bot.

Every rule is marked **cited** (read from the pinned server) or **authored** (a judgement made here). Mojang's code is not redistributable: this page names classes and methods and states their values. It copies no code.

## Instrument

- Server jar: piston-meta `downloads.server` for 1.21.11, sha1 `64bb6d763bed0a9f1d632ec347938594144943ed`; the bundled `META-INF/versions/1.21.11/server-1.21.11.jar`, sha256 `ec47239a8de246335e1d54f6ac319bd35641778eb4b6a6da06372840d02fcebc`.
- Mappings: piston-meta `downloads.server_mappings` for 1.21.11, sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`.
- Method: the bundled jar remapped to the Mojang names with SpecialSource 1.11.4 (shaded jar from Maven Central), then decompiled with Vineflower 1.10.1 (sha256 `b9b208e50793b64657a6b6292067526613f549de7405f9243624b02f4276e409`). Read sites are `Class.method`.
- Cross-check (second method, no shared configuration): the bot's own packet stream, read through mineflayer 4.37.1, on The Thing Across the Water. The run report's `shot_subjects` rows record what the client was sent and when (see "Measured").

## When an entity is sent

**Cited.** The server keeps one tracker per entity (`ChunkMap.TrackedEntity`). `TrackedEntity.updatePlayer(player)` pairs the entity with a player (sends its spawn and data packets, `ServerEntity.addPairing`) when all three hold, and unpairs it (`removePlayer`, a remove packet) otherwise:

1. The horizontal distance from **the player's position** to the entity is at most `min(effective range, view distance × 16)` blocks. The vertical distance is not read.
2. `Entity.broadcastToPlayer(player)` is true.
3. `ChunkMap.isChunkTracked(player, chunk)` holds for the entity's chunk: the chunk is inside the player's chunk tracking view and **is not pending send** (`PlayerChunkSender.isPending`).

- Effective range (`TrackedEntity.getEffectiveRange`): the entity type's `clientTrackingRange() × 16`, raised to a passenger's if one is larger, scaled by `MinecraftServer.getScaledTrackingDistance`. On a dedicated server that scale is `entity-broadcast-range-percentage × range / 100` (`DedicatedServer.getScaledTrackingDistance`; the property defaults to 100, clamped 10–1000, `DedicatedServerProperties`).
- `item_display` and `block_display`: `clientTrackingRange(10)`, `updateInterval(1)` (`EntityType`), so 160 blocks at the default percentage.
- View distance (`ChunkMap.getPlayerViewDistance`): the client's requested view distance clamped to `[2, server view-distance]`. The bot requests 32 (`harness/src/client-loaded.ts`), so the server's `view-distance` governs.
- Chunk tracking view (`ChunkTrackingView.isWithinDistance`): a chunk is in view when `max(0, |dx| − 2)² + max(0, |dz| − 2)² < view distance²`, in chunks from the player's chunk.

**Cited.** `updatePlayer` runs only at these moments, never on a timer:

- when the entity is added to the level (`ChunkMap.addEntity`, for every player);
- when the entity's section changes, and for every tracker when some player's section changes (`ChunkMap.tick`);
- for every tracker against one player, from `ChunkMap.move(player)` (through `ServerChunkCache.move`): called when the player moves (`ServerGamePacketListenerImpl`), when its camera changes (`ServerPlayer.setCamera`), and **every tick while the player's camera is another entity** (`ServerPlayer.tick`).

**Cited.** Sending a chunk pairs no entity (`PlayerChunkSender.sendChunk`). Chunks are sent at the end of the server tick (`MinecraftServer.tickChildren`, "send chunks", after the levels, the connections and the player list), paced by the client's acknowledgements: at most one unacknowledged batch until the first acknowledgement, then ten; the quota starts at 9 chunks per tick and follows the rate the client reports (`PlayerChunkSender.sendNextChunks`, `onChunkBatchReceivedByClient`). mineflayer acknowledges every batch (`lib/plugins/blocks.js`, `chunk_batch_finished`).

**Authored**, from the above: an entity in a chunk the client already holds, within range of the player, is sent in the very tick it is summoned. An entity whose chunk is sent at the end of tick `t` is sent no earlier than tick `t + 1`, at the next `updatePlayer` that reaches it; for a spectating player that is the `move` in `ServerPlayer.tick` of tick `t + 1`. Whether a chunk is sent by a given tick depends on the client's acknowledgements, so it is not a property of the build.

## What `view_range` changes

**Cited.** `Display.view_range` is synced entity data (`Display.DATA_VIEW_RANGE_ID`, default 1.0), read and written by `getViewRange`/`setViewRange` and the `view_range` NBT key. No server class other than `Display` refers to it: it plays no part in tracking. It is the client's render distance for the display.

## Where a spectating player stands

**Cited.** `ServerPlayer.setCamera(entity)` teleports the player to the camera entity's position, calls `ServerChunkCache.move(player)` (so every tracker is checked against the new position at once), and sends the camera packet. Every later tick, while the camera is alive, `ServerPlayer.tick` snaps the player to the camera (`absSnapTo`) and calls `move` again. So during a cutscene the tracking distance in rule 1 is measured from the lens, every tick.

## How a cutscene's packets reach the bot

**Cited.** A chat command from the client is queued on the server thread (`ServerGamePacketListenerImpl.handleChatCommand` → `tryHandleChat`) and runs between ticks (`MinecraftServer.waitUntilNextTick` → `runAllTasks`). The bot's game-time reading therefore lands after every packet of the tick it reports, and before any of the next.

**Cited** from the emitted functions (`cs_<bare>` and `cs_tick_<bare>`, `docs/reference/compiler.md`, the `cutscene` emission row): the start function switches every player to spectator, then summons the two lens entities (`item_display`), then schedules the driver one tick on; the driver's first run (driver tick 0) issues `spectate`. A body the same bundle places before the cutscene is summoned before the start runs. So, in the tick before driver tick 0, the bot is sent: the body (when it is in range of the player and its chunk is held), then the gamemode change, then the two lens spawns.

**Authored.** The lens entities share the `item_display` type with an assembly's root. An observer that treats any spawn of the body's type as a possible change of the body cannot place driver tick 0 whenever no reading lands between the start and the end of driver tick 0. A body is followed by its entity id.

## What the live check asserts

**Authored**, from the rules above. A framed body absent from the bot at its shot's first tick is a finding in both cases the server allows, and the record says which:

- its chunk had been sent to the bot by the end of the tick before: the server owed the body, so the content or the engine failed to place it within range;
- its chunk had not been sent: the server sends no entity in a chunk the client does not hold, so the shot opens before the client holds the place it frames. A real client is in the same position.

A tick no reading lands on is decided from the views on either side when they agree and nothing that could change the fact arrived between them. The start (the gamemode change) is one of those views. Anything else is undecided and counts as absent.

## Measured

The Thing Across the Water (content `demo/the-thing-across-the-water` @ `ef1be37`, manifest sha256 `6da98507936ce0b9a517328ab9c46a3e364c7dc214a5a0a65967c06ba00bf446`): the cutscene `obj/ring-the-bell/1` opens on `assembly/the-thing` at driver tick 0. The bundle summons the assembly (index 0) and starts the cutscene (index 1) in one function. One run on those bytes recorded 281 of 282 driver ticks read, with driver tick 0 the one unplaced, and the row false. The lens spawns sat between the start and the first reading. Read by entity id, with the start as a view, the same stream places driver tick 0. The repeated-run count is recorded in the commit that made this change.
