# spec-0100: The sculk family works — sensors, shriekers and catalysts enter a map as the vanilla mechanisms they are, pinned at rest, reaching nothing undeclared, and the walk proves them

- **Status**: Proposed
- **Ground**: written against engine `507a5e0e2` (`origin/main`). Read: `crates/delvec/src/admit/{allowlist,audit}.rs` (`DW0730`), `crates/dsl/src/blocks.rs` (`judge_at`), `crates/dsl/src/blockshape.rs` (`collision_class`, `Collision::PartialFloor`), `crates/delvec/src/compiler/{blockstate,assembled,light}.rs`, `crates/delvec/src/compiler/nav/world/body.rs` (`body_moves`, `mob_moves`), `crates/delvec/src/compiler/waypoints.rs` (`climbs`), `harness/src/{waypoints.ts,executor/walk.ts,load-window.ts}`, `tools/maintenance/{dump-faces.py,dump-collision-tops.py}` and `tools/maintenance/collision/FaceDump.java`, spec-0011 (the sculk row of its trigger inventory and its polling ruling), spec-0022, spec-0085, spec-0099 §7.
- **What it is for**: a room that listens — sensors set into the floor click at every footstep and set off a shrieker in the dark, with summoning off; a floor that turns to sculk and puffs on a story beat; a catalyst as a glowing fixture where nothing can ever die near it. On `507a5e0e2` the four acting blocks are refused by name (`DW0730`), and the engine can say nothing about what they would do.
- **Research**: §2 is this spec's record. Each statement is **cited** (the pinned server jar read by mapped name, its data files, a vendored table, the harness's pinned dependencies) or **authored** (this spec chooses).
- **Numbers**: spec `0100`. Four diagnostics, `DW-new-1`…`DW-new-4` (§5), allocated by the planner before implementation. No DSL surface change: a sculk block is declared the way every block is — a block state in a piece, a world-edit recipe or a `set-block`/`fill-region` effect — so no `dsl_version` is owed.
- **Non-goals**: a sensor powering a declared consumer (a door that opens to footsteps) — this spec makes a sensor's power reach *nothing*, and the consumer surface is a later spec; a calibrated sensor's frequency filter (§3.5); a sensor as a story trigger (§3.6, excluded); the warden in any form (§3.2); a waterlogged sensor or shrieker, which is the water rule's (spec-0038's open gap), not this spec's.

## 1. The thing, and the object class it belongs to

**Authored.** What a sculk block does is a fact about **a block state under the pinned game**; what its redstone can reach is a fact about **its neighbours' block classes**; whether anything can die near a catalyst is a fact about **where a body can be**. So:

- *Which states are at rest, and which one state is never admitted* belongs to `delvewright_dsl::blocks`, beside `judge_at`, as one rule every surface that admits a block state calls (§4.1).
- *Which blocks read a redstone signal, which are signal sources, which conduct* belongs to a pinned, measured table beside `collision-tops` and `faces` (§4.2).
- *What a sensor's power reaches* and *which cells a catalyst can hear a death from* are proofs over the assembled block map, in the compiler, reading the nav model's body relations and nothing private (§4.3, §4.4).
- *Which sensors the proven walk sets off* is derived from the exported route and carried on the waypoints, like a leg's `climbs` (§4.6); the bot asserts it (§4.7).

## 2. What vanilla does (cited)

Bytecode citations are of the pinned server jar (`versions.toml` `server_jar_sha256` `f83b8e09…1726`), read with `javap -c` and the official mappings (sha1 `5621e925…5955`) by mapped name. Tag citations are of the jar's `data/minecraft/tags/`.

### 2.1 Shape and light

`SculkSensorBlock.SHAPE` and `SculkShriekerBlock.SHAPE_COLLISION` are both `Block.column(16, 0, 8)`: a full-footprint box 8/16 high, which `crates/dsl/data/collision-tops-1.21.11.tsv` already records as `0 8` for `sculk_sensor`, `calibrated_sculk_sensor` and `sculk_shrieker`; `sculk_catalyst` is `0 16`. `blockshape::THIN_HEIGHT_16` is 8 and `collision_class` reads `top < THIN_HEIGHT_16` as thin, so all three are `Collision::PartialFloor(8)` — a bottom slab to the nav model — and the catalyst `FullCube`. Light: `compiler::light` already carries `sculk_catalyst` 6 and the two sensors 1 in every phase (the `Blocks` registration passes a constant `lightLevel`), the shrieker 0.

### 2.2 The sensor

- `SculkSensorBlockEntity$VibrationUser.getListenerRadius()` returns 8; `CalibratedSculkSensorBlockEntity$VibrationUser.getListenerRadius()` returns 16. `SculkSensorBlock.getActiveTicks()` 30; `CalibratedSculkSensorBlock.getActiveTicks()` 10.
- A vibration is a game event in `#game_event/vibrations` (55 members; `step` among them, `shriek` not). `VibrationSystem.VIBRATION_FREQUENCY_FOR_EVENT` gives `step`, `swim` and `flap` frequency 1. `SculkSensorBlockEntity$VibrationUser.canReceiveVibration` refuses a `block_place`/`block_destroy` at its own cell, a frequency-0 event, and any event unless `SculkSensorBlock.canActivate` (phase `inactive`).
- `VibrationSystem$User.isValidVibration`: the event must be in the user's listenable tag; an event from a spectator is refused; an event in `#game_event/ignore_vibrations_sneaking` (`step`, `swim`, `hit_ground`, `projectile_shoot`, `item_interact_start`, `item_interact_finish`) from an entity whose `isSteppingCarefully()` (`Entity`: `isShiftKeyDown()`) is true is refused. `VibrationSystem$Listener.isOccluded` clips lines between the two cell centres against `#block/occludes_vibration_signals` (wool).
- A player's footsteps are vibrations: `Entity.vibrationAndSoundEffectsFromBlock` posts `GameEvent.STEP`; `Entity.move` reaches it through `applyMovementEmissionAndPlaySound`, and `ServerGamePacketListenerImpl.handleMovePlayer` calls `ServerPlayer.move` for a player's own movement, so the server posts a player's steps.
- `onReceiveVibration` → `SculkSensorBlock.activate`: phase `active`, `power` = `VibrationSystem.getRedstoneStrengthForDistance(distance, radius)` = `max(1, 15 − floor(15 · distance / radius))`, a scheduled tick of `getActiveTicks()`, `updateNeighbours`, and the game event `sculk_sensor_tendrils_clicking` at the sensor's cell with the vibration's source entity as context.
- Block-entity NBT: `last_vibration_frequency`, `listener` (the pending vibration).

### 2.3 Redstone reach

- `SculkSensorBlock.isSignalSource` is true; `getSignal` returns `power` for every direction; `getDirectSignal` returns `power` only when the asked direction is `Direction.UP`, else 0. So the block **above** a sensor is strongly powered; its six neighbours are weakly powered.
- `SignalGetter.getSignal(pos, dir)` = that block's own `getSignal`, and, when the block `isRedstoneConductor`, the max with `getDirectSignalTo(pos)` — the strong power its neighbours hand it. `hasNeighborSignal(pos)` asks `getSignal` of the six neighbours. `BlockBehaviour$Properties` sets `isRedstoneConductor` to `lambda$new$6` = `BlockState.isCollisionShapeFullBlock` by default; a block overrides it only through `Properties.isRedstoneConductor(StatePredicate)`.
- A block reacts to power by **reading** a signal: `DoorBlock` calls `Level.hasNeighborSignal` (4 sites), `BaseRailBlock` (the parent of `RailBlock`) calls it once; `LeverBlock`, `SculkSensorBlock`, `SculkShriekerBlock` and `SculkCatalystBlock` call no `SignalGetter` method. `ObserverBlock` reads no signal: `updateShape` schedules its pulse on a neighbour's state change.
- `CalibratedSculkSensorBlock.getSignal` returns 0 toward its `facing` (the input side) and the sensor's power elsewhere. `CalibratedSculkSensorBlockEntity$VibrationUser.getBackSignal` reads `Level.getSignal(pos.relative(facing.getOpposite()), …)`; `canReceiveVibration` accepts every frequency when it is 0 and only the matching frequency otherwise.

### 2.4 The shrieker

- `SculkShriekerBlockEntity$VibrationUser.getListenerRadius()` 8; `getListenableEvents()` is `#game_event/shrieker_can_listen` = `sculk_sensor_tendrils_clicking` only. `canReceiveVibration` refuses while `shrieking` and unless `SculkShriekerBlockEntity.tryGetPlayer(source)` finds a player: the entity itself, its controlling passenger, a projectile's or an item entity's owner. **A mob's step sets off a sensor; only a player's reaches a shrieker.**
- `tryShriek(level, player)`: returns with no player or while `shrieking`; then `if (!canRespond(level) || tryToWarn(level, player)) shriek(level, player)`. `canRespond` = `can_summon` **and** difficulty not peaceful **and** gamerule `SPAWN_WARDENS`. `shriek`: `shrieking=true`, a scheduled tick of 90, `levelEvent(3007)` (the client plays `block.sculk_shrieker.shriek` and the particles), game event `shriek`.
- `SculkShriekerBlock.tick`: `shrieking=false`, then `tryRespond`: only under `canRespond` and a warning level above 0 does it `trySummonWarden` (`SpawnUtil.trySpawnMob(WARDEN, TRIGGERED, …, 20, 6, ON_TOP_OF_COLLIDER)`) or play the reply and `Warden.applyDarknessAround`. With `can_summon=false` a shrieker makes sound and particles and nothing else. `WardenSpawnTracker.tryWarn` is reached only through `tryToWarn`, which `canRespond` gates.
- `SculkBlock.getRandomGrowthState` places a catalyst-grown shrieker with `can_summon` = the spreader's `isWorldGeneration()`: false in a running world.
- Block-entity NBT: `warning_level`, `listener`.

### 2.5 The catalyst

`SculkCatalystBlockEntity$CatalystListener.getListenerRadius()` 8; `handleGameEvent` acts only on `entity_die` whose context source is a `LivingEntity` that has not `wasExperienceConsumed()`: it **always** blooms (`bloom=true`, `block.sculk_catalyst.bloom`, particles), and spreads — `SculkSpreader.addCursors(cell above the body, xp)` with `getExperienceReward` from the last damage source, placing blocks by the level's random — only when `shouldDropExperience()` and the reward is positive. Block state `bloom`; block-entity NBT `cursors`.

### 2.6 The range

`EuclideanGameEventListenerRegistry.getPostableListenerPosition`: a listener is visited when `BlockPos.containing(listenerPos).distSqr(BlockPos.containing(eventPos)) ≤ radius²` — **integer cells**, Euclidean. `BlockPositionSource.getPosition` is `Vec3.atCenterOf(pos)`, so the listener's cell is the block's own.

### 2.7 What a datapack can observe

`CriteriaTriggers` names 58 triggers; none fires on a block state change, a redstone signal or a game event — the two sculk-adjacent ones are `avoid_vibration` (a sneaking player's ignored vibration) and `kill_mob_near_sculk_catalyst`. `SetBlockCommand` posts no game event. The only read of a sensor's firing a datapack has is the block state, each tick, which spec-0011 rules a hack (`execute if block … [powered=true]` on the tick is polling).

### 2.8 The bot

mineflayer (the harness lockfile's pin) exposes `bot.blockAt` (the harness reads it in `load-window.ts`), a `blockUpdate` event with the new state's properties, an `entityEffect` event, `entitySpawn`, and the raw `world_event` packet (id, position) on `bot._client`. prismarine-physics takes a block's collision from minecraft-data's shape table, where the three 8/16 blocks are half-height boxes. A critical-path step marked `sneak: true` is walked sneaking (`harness/src/movement.ts`).

## 3. Scope

### 3.1 What is admitted, and how it is declared (authored)

`sculk_sensor`, `calibrated_sculk_sensor`, `sculk_shrieker` and `sculk_catalyst` join `sculk` and `sculk_vein` in the default allowlist, and the four reasons recorded against them in `allowlist.rs` and `docs/reference/tools.md` §3 are replaced by the rules below. A creator declares one by writing its block state wherever a block is written — a generator's piece, a community piece, a world-edit `fill`/`replace`/`scatter` recipe, a `set-block` or `fill-region` effect — and **the state is the declaration**: `minecraft:sculk_shrieker[can_summon=false]`, or the bare id, whose defaults (`crates/dsl/data/block-defaults-1.21.11.json`) are the rest state.

### 3.2 A shrieker that can summon is refused (authored, over §2.4)

`can_summon=true` is refused everywhere a block state is entered (`DW-new-1`). The warden is a mob no wave declares, placed by the game's random up to 6 cells from the shrieker, with a warning level the game keeps per shrieker and per player across sessions — nothing a proof can seat, count or make deterministic (ADR-0006) — and Darkness is applied to every player nearby. Setting the gamerule instead is not taken: the pin is the state, one authority.

### 3.3 Every sculk block enters at rest (authored, over §2.2, §2.4, §2.5)

A sculk block whose consequential state is not its rest value is refused (`DW-new-2`): `sculk_sensor_phase` other than `inactive`, `power` other than 0, `shrieking=true`, `bloom=true`; and at admission a block-entity `nbt` carrying a pending `listener` event, a non-zero `last_vibration_frequency` or `warning_level`, or a non-empty `cursors`. A piece saved mid-click would replay the click on load; a `set-block` of `bloom=true` paints a state the game never schedules back.

### 3.4 Containment: a sensor's power reaches nothing (authored, over §2.3)

A sensor's **reach** is its six neighbours and, when the cell above it conducts (§4.2), that cell's five other neighbours. Every cell in the reach must hold a block that does not read a signal (`DW-new-3`, first arm). Nothing in the DSL consumes redstone (spec-0022: redstone keeps one job, the trigger, and a sensor is not one — spec-0011 ruled it not player-distinct), so there is no declared consumer to allow, and the rule has no exception. Redstone dust, diodes and observers never reach the assembled world: `DW0730` refuses them at admission and they are not block states a recipe may carry.

### 3.5 A calibrated sensor's filter is pinned open (authored, over §2.3)

The cell on a calibrated sensor's input side (`pos.relative(facing.getOpposite())`) must hold a block that is neither a signal source nor a conductor (`DW-new-3`, second arm), so `getBackSignal` is 0 and the sensor hears every frequency, like a plain one with range 16. A declared frequency needs a signal of exact strength at that cell — a `redstone_block`, dust or a diode — none of which this engine admits; it is excluded until a declared-strength source is first-class.

### 3.6 A sensor is not a story trigger (authored, over §2.7)

Excluded. Vanilla gives a datapack no event for a sensor's firing (§2.7); the one read is a per-tick block-state poll, which spec-0011 rules a hack, and `#minecraft:tick`-driven detection of a *block* is what the no-hack doctrine names. The story is driven by what already is first-class — `approach`, `reach-anchor`, `interact` — and a sensor is ambience beside it, exactly spec-0011's verdict ("admitted only as pure ambience").

### 3.7 The catalyst is admitted where nothing can die near it (authored, over §2.5, §2.6)

A catalyst blooms on any living death within its range and rewrites blocks by the game's random on a death that drops experience — the world the proofs judged changes mid-fight (ADR-0006). It is admitted only where no body can ever be within its range: every cell any body relation can put a body in (§4.4) must have integer-cell `distSqr > 64` to the catalyst's cell, else `DW-new-4`. A catalyst so placed is a fixture of light 6 that provably never acts; the diagnostic names the nearest body cell, so a creator knows exactly how far to move it.

### 3.8 The pulse of a floor is already a verb (authored)

A floor that turns to sculk and puffs on a beat is `fill-region` with `minecraft:sculk` (inert, already admitted) plus `particle` (`sculk_charge_pop` and `sculk_soul` take no options; `sculk_charge` and `shriek` take options and are refused by `DW0941`) plus `play-sound` (`block.sculk_catalyst.bloom`, `block.sculk.spread`, `block.sculk_shrieker.shriek` are in the pinned sound registry). Nothing is added for it; the demo level (§6) shows the spelling.

## 4. The model

### 4.1 One rest rule (authored)

`delvewright_dsl::blocks::sculk_rest(state) -> Result<(), SculkFault>` judges a full block state (defaults filled from `block-defaults`) and, given one, the block-entity NBT. It is called at every surface a block state enters by: the admission audit per palette entry (beside `judge_at`), the stage-7 recipe/item validator and the `set-block`/`fill-region`/`prop` id validator (beside `DW0193`'s suffix parse — the one shared parse rule), and once more over the whole assembled block map at build, so a piece no audit ran (a generator's, `detail`'s, `sculpt`'s) is judged too. One function, one message; the entry-point calls refuse where the state is typed, the build call cannot be bypassed.

### 4.2 The redstone table (authored instrument over cited rules)

`crates/dsl/data/redstone-1.21.11.tsv`, written by `tools/maintenance/dump-redstone.py` with `tools/maintenance/collision/RedstoneDump.java`, importing `dump-collision-tops.py`'s pin, fetch, mapping and collapse steps as `dump-faces.py` does; the header names the jar sha256 and the mappings sha1. One row per block id, three measured columns:

- `conductor`: `always`, `never` or `shape`, by evaluating `BlockStateBase.isRedstoneConductor(EmptyBlockGetter.INSTANCE, BlockPos.ZERO)` against `isCollisionShapeFullBlock` over every state of the block (§2.3: the default is the shape, an override is per block). `shape` is decided per cell from the collision table the nav model already reads.
- `signal_source`: `BlockStateBase.isSignalSource()` over every state, collapsed.
- `reads_signal`: whether any method of the block's class or an ancestor below `BlockBehaviour`, or of its block-entity class when it is an `EntityBlock`, invokes one of `SignalGetter`'s seven methods (`getSignal`, `getDirectSignal`, `getDirectSignalTo`, `getControlInputSignal`, `hasSignal`, `hasNeighborSignal`, `getBestNeighborSignal`) on a receiver assignable to `SignalGetter` — read from `javap -c` of the inner jar with the method names resolved from the mappings, never written down obfuscated. Over-approximation is the safe direction: a block that reads power anywhere is one whose behaviour power can change.

`blockshape::redstone_conductor`, `redstone_source`, `redstone_reader` read it. Spot checks the table test asserts: `oak_door`, `iron_door`, `rail`, `bell`, `redstone_lamp`, `copper_bulb`, `big_dripleaf`, `calibrated_sculk_sensor` read a signal; `stone`, `lever`, `stone_pressure_plate`, `sculk_sensor`, `sculk_shrieker`, `sculk_catalyst` do not; `lever`, `sculk_sensor`, `lightning_rod` are sources, `stone` is not; `stone` conducts by `shape`, `glass` `never`; the row count equals the pinned block registry's.

### 4.3 The containment proof (authored)

`compiler::sculk::prove_reach(map)`: for every sensor cell of the assembled block map, compute the reach (§3.4) and refuse any reach cell whose block `redstone_reader`s; for every calibrated sensor, refuse an input cell that is a source or conducts. Run over the world at load and after every runtime write set the derived-world sequence already carries (a `fill-region` that lays an iron door beside a sensor is the same defect later). Binding line: `sculk: <s> sensor(s) (<c> calibrated), <r> reach cell(s) checked, <k> shrieker(s), <t> catalyst(s)`.

### 4.4 The catalyst proof (authored)

`compiler::sculk::prove_catalysts(map, nav)`: the body cells are the closure of `World::body_moves` from the party's entry and every teleport target, united with the closure of `World::mob_moves` from every wave seat and spawn cell (the relations `DW0922`/`DW0923`/`DW0924` already take); a catalyst with any body cell at integer `distSqr ≤ 64` is `DW-new-4`, naming the nearest. Binding line: `sculk: <t> catalyst(s), nearest body cell at distSqr <d>`; `0 catalysts` is stated.

### 4.5 Nav, light, determinism (authored over §2.1)

Nothing new is modelled: the three 8/16 blocks are already `PartialFloor(8)` and the catalyst `FullCube`; light is already in `compiler::light`. The tests in §Acceptance pin that reading. Determinism holds because every sculk block is at rest at load (§3.3), a sensor's power reaches nothing (§3.4), a shrieker with `can_summon=false` changes no block and spawns nothing (§2.4), and a catalyst can hear no death (§3.7). The written-world comparison (`DW0955`) is of the save at load with no player present, so the rest states match cell for cell.

### 4.6 The walk's vibrations (authored over §2.2, §2.4, §2.6)

Each exported leg carries `vibrations[]` — `{sensor: [x,y,z], shriekers: [[x,y,z], …]}` — emitted only when present: a sensor is **predicted** for a leg when the leg is not a `sneak` leg and a cell of its proven route has integer `distSqr ≤ 49` to the sensor (one block inside the radius, so the bot's deviation from the route cannot falsify it); a shrieker is predicted for that sensor when its cell has `distSqr ≤ 64` to the sensor's and no `#occludes_vibration_signals` block lies in the box the two cells span (a superset of every line `isOccluded` clips). A sensor or shrieker no leg predicts is stated in the build's binding line as unpredicted, not refused: a sensor off the walk is a legitimate declaration.

### 4.7 The harness (authored over §2.8)

For each predicted sensor, from the leg's start to three seconds after its end, the bot records a `blockUpdate` at the sensor's cell whose new state has `sculk_sensor_phase=active`, and for each predicted shrieker a `world_event` packet with id 3007 at the shrieker's cell. Over the whole run it records every `entityEffect` of `minecraft:darkness` on itself and every `entitySpawn` of type `warden`. Binding line per leg: `[sculk] <leg>: <n> sensor(s) predicted, <n> heard; <m> shrieker(s) predicted, <m> heard`; at the end: `[sculk] darkness 0, warden 0`. A predicted event not heard fails the step; a darkness effect or a warden fails the run. Assertions and navigation only: the harness predicts nothing itself.

## 5. Diagnostics

| Code | Meaning | Tier |
| --- | --- | --- |
| `DW-new-1` | A sculk shrieker is declared with `can_summon=true` (§3.2), naming where the state was entered and the cell. | Build (exit 3); the admission audit reports it as an error. |
| `DW-new-2` | A sculk block is not at rest (§3.3): the property or NBT field and its value, the cell, where it was entered. | Build (exit 3); admission as above. |
| `DW-new-3` | A sculk sensor's redstone neighbourhood is not inert (§3.4, §3.5): first arm, a reach cell holds a block that reads a signal (sensor cell, reach cell, block); second arm, a calibrated sensor's input cell holds a source or a conductor. | Build (exit 3). |
| `DW-new-4` | A sculk catalyst can hear a death (§3.7): the catalyst cell, the nearest body cell and its `distSqr`. | Build (exit 3). |

`DW-new-1` and `DW-new-2` are declared in `delvewright_dsl::blocks` (their rows on `docs/reference/dsl/blocks.md`); `DW-new-3` and `DW-new-4` in `compiler::sculk` (`docs/reference/delvec/compiler/sculk.md`).

## 6. The gallery's obligation, and the demo level

The gallery hall gains a **listening floor** on the far side of the barred wall, on the critical walk: a `sculk_sensor` set into a floor cell within one block of the route, a `calibrated_sculk_sensor[facing=north]` within the same stretch with air behind it, a `sculk_shrieker[can_summon=false]` within `distSqr ≤ 64` of the plain sensor, nothing in either sensor's reach that reads a signal, and a `sculk_catalyst` where §4.4 admits one (the proof names the nearest body cell; the implementer finds the cell, and the build line states it). Four probes, each a one-cell world-edit `fill` on a scratch copy — so the recipe entry point of §4.1 is the one exercised: `a-shrieker-that-can-summon` (`can_summon=true` → `DW-new-1`), `a-sensor-caught-mid-click` (`sculk_sensor_phase=active` → `DW-new-2`), `an-iron-door-beside-a-sensor` (`DW-new-3`), `a-catalyst-over-the-floor` (`DW-new-4`). The README names the floor and the probes.

`docs/demo-levels.md` gains the row **The Listening Hall** (0100): a sealed hall whose floor is set with sensors, so every footstep clicks and a shrieker answers from the dark with summoning off; a calibrated sensor at the far door that hears from sixteen blocks; one story beat that turns the floor to sculk and puffs it (`fill-region` + `particle` + `play-sound`, §3.8); a catalyst sealed in a glass well too deep for any body to reach, glowing. Status pending. The capability is confirmed on that level, never on a campaign's renders.

## 7. Decisions

1. **The four acting blocks are admitted by state, not by name** (§3.1): the state is the declaration and the one rest rule judges it at every entry and over the assembled world.
2. **`can_summon=true` is refused** (§3.2); the gamerule is not the pin.
3. **A sensor's power reaches nothing** (§3.4) and **a calibrated sensor hears every frequency** (§3.5); a declared consumer and a declared frequency are later specs.
4. **A sensor is not a story trigger** (§3.6), on spec-0011's standing ruling and §2.7's enumeration.
5. **The catalyst is admitted under the body-reach proof** (§3.7).
6. **Redstone facts are measured, not typed** (§4.2), with the instrument and spot checks named.
7. **The walk's vibrations are predicted by the compiler and asserted by the bot** (§4.6, §4.7); a device off the walk is a stated zero binding.
8. **No DSL surface**: no `dsl_version`.

## Acceptance criteria

1. `crates/dsl/data/redstone-1.21.11.tsv` exists, written by `tools/maintenance/dump-redstone.py` from the pinned jar (header names the jar sha256 and the mappings sha1); `blockshape::tests::the_redstone_table_reads_the_jar` asserts every spot check of §4.2 and that the row count equals the pinned block registry's. *Vacuous if* the table were empty or unread: `oak_door` reads, `stone` does not, and a missing row answers `false` to both.
2. `blocks::tests::a_sculk_block_is_judged_by_its_state`: `sculk_shrieker[can_summon=true]` is `DW-new-1`; `sculk_sensor[sculk_sensor_phase=active]`, `[power=3]`, `sculk_shrieker[shrieking=true]`, `sculk_catalyst[bloom=true]`, and a sensor NBT with a `listener.event` are each `DW-new-2` naming the field; the bare ids and `sculk_shrieker[can_summon=false]` pass; `sculk` and `sculk_vein` are not judged. *Vacuous if* defaults were not filled: the bare `sculk_shrieker` would have no `can_summon` to pass on.
3. `admit_audit::the_sculk_family_passes_at_rest`: a piece carrying all six sculk blocks at rest is admitted with zero `DW0730`; the same piece with `can_summon=true` is refused `DW-new-1`, with `sculk_sensor_phase=active` `DW-new-2`. *Vacuous if* the allowlist still named them: the rest piece would be `DW0730`.
4. A world-edit `fill` recipe and a `set-block` effect naming `sculk_shrieker[can_summon=true]` are each refused `DW-new-1` at validation, before any world is assembled; `fill-region` with `sculk_catalyst[bloom=true]` is `DW-new-2`. *Vacuous if* only the build call judged: the message would name the assembled cell, not the document path.
5. `compiler::sculk::tests::a_sensor_reaches_its_neighbours_and_the_cell_above_conducts`: a sensor with an iron door beside it is `DW-new-3`; with stone above it and an iron door beside the stone, `DW-new-3`; with glass above it and the same door, passes (glass `never` conducts); with a calibrated sensor facing north and a lever behind it, `DW-new-3` second arm; with air behind it, passes. *Vacuous if* the reach ignored the conductor: the stone case would pass.
6. `a_catalyst_is_refused_within_reach_of_a_body`: a catalyst 8 cells from a route cell is `DW-new-4` naming that cell and `64`; at 9 it passes; a catalyst 8 cells from no route cell but 7 from a wave seat is `DW-new-4`. *Vacuous if* only the route were read: the wave case would pass.
7. `the_sculk_blocks_are_half_floors`: `collision_class` is `PartialFloor(8)` for the three 8/16 blocks and `FullCube` for the catalyst; a body steps from a floor onto a sensor cell and back (`nav::world` route exists, step cost that of a bottom slab). *Vacuous if* the tsv were unread: the default would be `FullCube` and no route would exist.
8. On the gallery primary point built by this branch: the build prints `sculk: 2 sensor(s) (1 calibrated), <r> reach cell(s) checked, 1 shrieker(s), 1 catalyst(s)` with `r` ≥ 12, and `nearest body cell at distSqr <d>` with `d > 64`; `validation/critical-path-waypoints.json` carries `vibrations` on at least one leg naming the plain sensor's cell and the shrieker's. The same documents and pieces built by `507a5e0e2`'s `delvec` are refused. *Vacuous if* no leg predicted the sensor: the harness line would read `0 predicted`.
9. Each of the four probes in §6 is refused by `delvec build` with its named code, each naming the filled cell. *Vacuous if* a probe perturbed nothing the rule reads: each changes one block state.
10. `harness/test/waypoints.test.ts` parses a leg's `vibrations`, refuses a sensor cell that is not within 8 of any waypoint of its leg, and refuses a shrieker not within 8 of its sensor; `harness/test/executor.test.ts` asserts a predicted shrieker whose 3007 is not heard fails the step and a `darkness` effect fails the run. *Vacuous if* no leg carried a vibration: the test asserts exactly the gallery's count.
11. The CI `gallery bot` job passes on this branch with `[sculk] <leg>: 1 sensor(s) predicted, 1 heard; 1 shrieker(s) predicted, 1 heard` for the listening floor's leg and `[sculk] darkness 0, warden 0` at the end. *Vacuous if* the bot sneaked that leg: the compiler would have predicted nothing and the line would read `0 predicted`.
12. `crates/delvec/src/admit/allowlist.rs` lists the four blocks and its comment points at this spec's rules; `docs/reference/tools.md` §3 states the same and lists `dump-redstone.py` under maintenance; `docs/reference/compiler.md` describes the rest rule, the reach, the catalyst proof and the `vibrations` export; `docs/reference/dsl/blocks.md` carries `DW-new-1`/`DW-new-2` and `docs/reference/delvec/compiler/sculk.md` `DW-new-3`/`DW-new-4`; `docs/reference/playtest-methodology.md` describes the `[sculk]` lines; `docs/demo-levels.md` carries the row; the gallery README names the listening floor and the four probes.
