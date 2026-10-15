# How a vanilla body moves — the pinned game's movement rules, read from the jar

For whoever implements or audits the reachable-surface model (spec-0113): what
Minecraft Java 1.21.11 does, tick by tick, when a player body walks, sprints,
jumps, falls, swims or stands on a block that changes any of those. Every rule is
**cited** to the class and method it was read from, or marked **derived** (arithmetic
over cited rules) or **simulated** (a number two independent implementations of the
cited rules agree on). Nothing here is a recurring check: a vanilla fact is
recorded once, here, and the engine's tests hold the engine's own simulator to it.

## 0. What was read, and how

- The pinned server jar, `versions.toml` `server_jar_sha256` `f83b8e09…1726` (the
  bundler), whose inner `META-INF/versions/1.21.11/server-1.21.11.jar` has sha256
  `ec47239a…fcebc`; the official server mappings for 1.21.11 (sha1
  `5621e925…5955`, from piston-meta). Movement is computed on the **client**, so
  the two client-only classes below were read from the pinned 1.21.11 client jar
  (sha1 `ba2df812…f5bd`) with the client mappings (sha1 `031a68be…232a`). Both jars
  were remapped to Mojang names and decompiled for reading only; neither the
  remapper nor the decompiler is part of this repository, and no game code is.
- Two independent implementations were then driven over the same rigs to put
  numbers beside the rules: a scratch tick simulator written from the citations
  below, and prismarine-physics 1.11.1 (MIT), the client physics the harness bot
  runs, at the harness lockfile's pin. On every rig in §9 they agree at every tick
  to four decimals of position and velocity, and they agree with the live
  measurement in `docs/notes/jump-arc-model.md` §2 (a sprint jump pressed at
  `x = 0.984` lands at `4.613`).

## 1. The tick, in order

**Cited** — `LivingEntity.aiStep`, with `LocalPlayer.applyInput` replacing the
base `applyInput` for the player whose client this is.

1. `noJumpDelay` counts down if positive.
2. Small velocities are zeroed: for a player, the horizontal pair when its squared
   length is under `9e-6`; `y` when `|y| < 0.003`.
3. **Input** (`LocalPlayer.applyInput` → `modifyInput`): `xxa` (strafe) and `zza`
   (forward) are taken from the move vector of §2; `jumping` is the jump key.
4. **Jump** — when `jumping`: in water or lava above the fluid-jump threshold the
   body gets `+0.04` vertical a tick (`jumpInLiquid`); otherwise, when `onGround`
   and `noJumpDelay == 0`, `jumpFromGround` runs and `noJumpDelay = 10`. With the
   key released `noJumpDelay = 0`.
5. **Travel** (`travel`): in water or lava (`shouldTravelInFluid`) the fluid arm
   of §6; otherwise `travelInAir` (§3), which calls `Entity.move` (§4) in its
   middle.
6. `applyEffectsFromBlocks`: `stepOn` of the block under the body, then the
   `entityInside` effects of every block the body's movement passed through this
   tick (cobweb, honey slide, powder snow, sweet berries).

## 2. Input

- **Cited** `KeyboardInput.tick`: the move vector is `(left−right, forward−back)`
  as `±1`/`0`, **normalised**.
- **Cited** `LocalPlayer.modifyInput`: the vector is scaled by `0.98`; while using
  an item, by the item's speed multiplier; while moving slowly (crouching or
  crawling), by the `sneaking_speed` attribute (`0.3`, `Attributes`); then
  `modifyInputSpeedForSquareMovement` rescales it toward the unit square, so a pure
  diagonal has magnitude `min(0.98·√2, 1) = 1.0` against `0.98` straight ahead.
- **Cited** `LivingEntity.applyInput` (the base, which a mob and a mannequin use):
  `xxa` and `zza` are multiplied by `0.98` (`INPUT_FRICTION`).
- **Cited** `Entity.getInputVector`: a vector longer than 1 is normalised; the
  result is scaled by the speed of §3 and rotated by the body's yaw through
  `Mth.sin`/`Mth.cos`.
- **Cited** `Mth.sin`: a 65536-entry `float` table, `SIN[(int)((long)(x ·
  10430.378350470453) & 65535)]`, `cos` the same at `+16384`. At a cardinal yaw the
  table holds exactly `0` and `±1`.
- **Sprint** (client only). **Cited** `LocalPlayer.aiStep`/`canStartSprinting`: a
  body starts sprinting when it is not sprinting, has forward impulse (`moveVector.y
  > 1e-5`), is not moving slowly, is not using an item, and `isSprintingPossible`:
  not mobility-restricted, enough food (`FoodData.hasEnoughFood`: food level `> 6`)
  and not in shallow water. **Cited** `shouldStopRunSprinting`: it stops when any of
  those fails, when forward impulse is lost, or on a horizontal collision that is
  not minor. **Cited** `LivingEntity.setSprinting`: sprinting adds the transient
  `movement_speed` modifier `+0.3` `ADD_MULTIPLIED_TOTAL`.

## 3. Air travel: acceleration, friction, gravity

**Cited** `LivingEntity.travelInAir`, `handleRelativeFrictionAndCalculateMovement`,
`getFrictionInfluencedSpeed`, `Player.getFlyingSpeed`, `Player.getSpeed`.

- `f` = the friction of the block that affects the body's movement (§3.1) while
  `onGround`, else `1.0`; `f1 = f · 0.91`.
- Acceleration this tick: on the ground `speed · 0.21600002 / f³`, with `speed` the
  `movement_speed` attribute (`0.1` for a player, `Player.createAttributes`; `0.13`
  sprinting); in the air `0.02`, or `0.025999999` sprinting. The accelerated input
  vector is added to the velocity (`moveRelative`), the climbable clamp of
  spec-0099 is applied, then **`Entity.move` runs with that velocity** (§4).
- After the move, gravity: `y −= 0.08` (`Attributes.GRAVITY` default `0.08`;
  `getEffectiveGravity`), then drag: `(x · f1, y · 0.98, z · f1)`.
- **Derived**: on default ground (`f = 0.6`) the acceleration is `0.1`/tick walking
  and `0.13` sprinting and the horizontal drag factor is `0.546`; in the air the
  drag factor is `0.91`. The fall law `v′ = (v + 0.08) · 0.98` has fixed point
  `3.92` blocks a tick; the 15th tick of a fall from rest is the first to move a
  whole block (`8.58` fallen before it), so a one-block cell catches a body that
  has fallen at most 7 (`metrics::climb_catch_fall_blocks`).

### 3.1 Which block's friction, speed and jump factors apply

- **Cited** `Entity.getBlockPosBelowThatAffectsMyMovement` = `getOnPos(0.500001)`:
  the main supporting block if it is a fence, wall or fence gate, else the block at
  `floor(y − 0.500001)` under the body's position — for a full block, the block
  the body stands on; on a bottom slab (feet at the slab's `y + 0.5`), the block
  *beneath* the slab, so a slab laid over ice slides. The simulator reads it so
  (`game_below`); on a fence, wall or gate top it is the barrier itself.
- **Cited** `BlockBehaviour.Properties`: defaults `friction 0.6`, `speedFactor 1.0`,
  `jumpFactor 1.0`. **Cited** `Blocks`: `ice`, `packed_ice` and `frosted_ice`
  `friction 0.98`; `blue_ice` `0.989`; `slime_block` `0.8`; `soul_sand`
  `speedFactor 0.4`; `honey_block` `speedFactor 0.4`, `jumpFactor 0.5`. No other
  block sets any of the three.
- **Cited** `Entity.getBlockSpeedFactor` (applied at the end of every `move`):
  the factor of the block at the body's feet cell, else of the block below; the
  velocity's horizontal pair is multiplied by it. `Entity.getBlockJumpFactor`: the
  feet cell's factor, else the block below's.

## 4. Collision, stepping, landing, head-hitting

**Cited** `Entity.move`, `Entity.collide`, `Entity.collideWithShapes`,
`Direction.axisStepOrder`, `VoxelShape.collideX`, `Entity.collectCandidateStepUpHeights`,
`LivingEntity.maxUpStep`, `Block.updateEntityMovementAfterFallOn`.

- The movement is resolved **one axis at a time, `y` first**, then the larger of
  the two horizontal components, then the other, each against the body's box as
  the previous axes left it. An interval that merely touches a shape (within
  `1e-7`) does not collide with it.
- **Step-up**: with `step_height` `0.6` (`Attributes.STEP_HEIGHT`), when the body is
  `onGround` or was stopped moving down, and a horizontal axis was blocked, every
  collider edge height up to `0.6` above the body's feet is tried as a lift; the
  first lift that moves the body further horizontally is taken.
- `onGround` is set after the move to "the `y` move was blocked while moving down",
  **judged on the box before the horizontal move** — so a body keeps `onGround` for
  the tick in which its box leaves a ledge, and can jump from up to one tick's
  travel past the edge (**simulated**, §9).
- A vertical collision in either direction sets the vertical velocity to `0`
  (`Block.updateEntityMovementAfterFallOn`; slime and honey override it, §7); a
  horizontal collision zeroes the blocked component, and, on the client, ends the
  sprint unless the collision is minor (§2).
- **Derived** from the three rules above and **simulated**: a sprinting body walks
  straight across a one-column gap without jumping (the lagged `onGround` plus the
  step-up onto the far block), and falls into a two-column gap; a walking body
  falls into either.

## 5. Jump

**Cited** `LivingEntity.jumpFromGround`, `getJumpPower`, `getJumpBoostPower`.

- The vertical velocity becomes `max(jump_strength · jumpFactor + boost, y)`, with
  `jump_strength` `0.42` (`Attributes`), `jumpFactor` from §3.1 and `boost = 0.1 ·
  (amplifier + 1)` under Jump Boost. **Sprinting adds `0.2` along the facing**
  (`−sin(yaw) · 0.2, cos(yaw) · 0.2`).
- **Derived**: the apex is `1.2522` blocks over the launch surface and the body is
  airborne for twelve ticks (`metrics::MAX_JUMP_RISE_16`, `JUMP_AIRBORNE_TICKS`).
- **Simulated** (§9): pressed on the last supported tick after a run-up of two or
  more cells, a sprint jump carries the feet centre `3.63` blocks from the press
  to the touchdown and clears four columns of air on the flat; from a standing
  start inside the launch cell, three; a walking jump carries `2.04` and clears
  two. Up one block: three columns sprinting; down one: four; down two or three:
  five; down five: six; down ten: seven. Successive jumps across a chain of
  one-wide pillars neither gain nor lose reach: the run-up speed is already the
  terminal speed, so momentum carried across jumps is bounded by the single-jump
  figure.
- **Head-hitters** (**simulated**): a lid two cells over the launch floor stops the
  jump at `+0.2` and the body crosses nothing a sprint-walk does not; a lid three
  cells over clamps the apex to `1.20` and the sprint jump still clears three
  columns; four cells over, the full arc.

## 6. Water and lava

**Cited** `Entity.updateFluidHeightAndDoFluidPushing`,
`updateInWaterStateAndDoWaterCurrentPushing`, `LivingEntity.travelInWater`,
`travelInLava`, `getFluidFallingAdjustedMovement`, `jumpOutOfFluid`,
`Entity.getFluidJumpThreshold`, `FlowingFluid.getFlow`, `LivingEntity.baseTick`.

- A body is **in water** when some water cell's surface (`cell y + fluid height`)
  is at or above the bottom of its box deflated by `0.001`; being in water resets
  the fall distance every tick.
- In water: acceleration `0.02` (raised toward `movement_speed` by
  `water_movement_efficiency`, `0` by default), then `move`, then the velocity is
  multiplied by `(0.8, 0.8, 0.8)` — `0.9` horizontally while sprinting; then, not
  sprinting, it sinks by `gravity / 16 = 0.005` a tick, settling at `−0.003`
  (`getFluidFallingAdjustedMovement`). Holding jump gives `+0.04` a tick when the
  fluid is deeper than the threshold `0.4` at the body (§1 step 4); in water no
  deeper than `0.4` over a floor the body jumps as on land.
- **Climbing out**: after the move, if the body collided horizontally and the space
  `0.6` above its feet is free, its vertical velocity becomes `0.3`
  (`jumpOutOfFluid`). A climb out of one block over the top water cell is this
  rule's consequence, and the simulator flies it rather than holding it as a
  constant.
- **Current**: each water cell the body overlaps contributes its flow vector
  (`FlowingFluid.getFlow`, from height differences to its four neighbours, `−6`
  vertical beside a falling column against a solid face), averaged and scaled by
  `0.014`; a body nearly at rest is pushed by at least `0.0045`.
- In lava: acceleration `0.02`, velocity `× (0.5, 0.8, 0.5)` below the jump
  threshold and `× 0.5` above it, gravity `/ 4`; the flow scale is `0.00233`.
- **Air**: `300` ticks (`Entity.getMaxAirSupply`), one less a tick with the eyes
  under water, two points of drowning damage every twenty ticks once it is spent
  (`LivingEntity.baseTick`, `shouldTakeDrowningDamage`); so an unarmoured body at
  full health has twenty-five seconds under water before it dies.
- **Swimming** (`Player.travel`, `Avatar.POSES`): sprinting in water puts the body in
  the `0.6 × 0.6` swimming pose, steered by the look pitch.

## 7. Blocks that change movement

**Cited**, each from its block class.

- **Slime** (`SlimeBlock`): no fall damage unless sneaking; on landing the vertical
  velocity is negated (`× 1.0` for a living entity) unless sneaking; stepping on it
  with `|y| < 0.1` multiplies the horizontal velocity by `0.4 + 0.2·|y|` unless
  sneaking; friction `0.8`.
- **Honey** (`HoneyBlock`): collision box 14 wide, 15 high (`Block.column(14, 0, 15)`);
  fall damage `× 0.2`; a body beside it, not on the ground, within `0.4375 + half
  width` of its centre on either axis and moving down faster than `0.08`, slides at
  `−0.05` a tick with its fall distance reset and its horizontal speed throttled;
  `speedFactor 0.4`, `jumpFactor 0.5`.
- **Soul sand** (`SoulSandBlock`): collision 14 high (`Block.column(16, 0, 14)`);
  `speedFactor 0.4`.
- **Cobweb** (`WebBlock`): `makeStuckInBlock(0.25, 0.05, 0.25)` — the next move is
  scaled by it and the velocity then zeroed — `(0.5, 0.25, 0.5)` for a body under
  Weaving; stuck resets the fall distance. **Powder snow** (`PowderSnowBlock`):
  `makeStuckInBlock(0.9, 1.5, 0.9)`, a floor only to a body in leather boots.
- `#fall_damage_resetting`: `#climbable`, `sweet_berry_bush`, `cobweb` — a move of
  a block or more through one of these forgets the fall (`Entity.move`).
- **Climbables**: spec-0099 §2 is the record; nothing is restated here.
- Not read: bubble columns, magma (damage, not movement), elytra, riptide, wind
  charges, ender pearls and chorus fruit — none moves a body under the model
  spec-0113 states, and each is refused there by name while it does not.

## 8. The body

- **Cited** `Avatar.POSES`: standing `0.6 × 1.8`, eye `1.62`; crouching `0.6 × 1.5`,
  eye `1.27`; swimming (and crawling) `0.6 × 0.6`, eye `0.4`.
- **Cited** `Player.updatePlayerPose`: the pose is the one wanted (crouching when
  sneaking, swimming when swimming, else standing) if the body fits it where it
  is; else crouching if that fits; else swimming. A body under a ceiling lower than
  its standing height is therefore crouched or crawling by the geometry alone, and
  crawls under one block of headroom.
- **Cited** `Player.maybeBackOffFromEdge`: a sneaking body's horizontal move is
  trimmed in steps of `0.05` until it would not fall `step_height` or more — it
  stops at the edge and never leaves it; it does not stand further out than a
  walking body can.
- **Fall damage** — **cited** `Entity.checkFallDamage` (the fall distance grows by
  every downward move while not in water, and is spent on landing),
  `LivingEntity.calculateFallDamage`/`calculateFallPower`: `floor((d + 1e-6 −
  safe_fall_distance) · multiplier · fall_damage_multiplier)`, `safe_fall_distance`
  `3.0`, the attribute `1.0`. **Derived**: `22` blocks deal 19 and `23` deal 20 to
  an unarmoured body at `20` health (`metrics::unarmoured_survivable_fall_blocks`).
  At a non-integer distance this floor is not `metrics`'s documented `ceil(d − 3)`:
  `3.5` blocks deal nothing.
- **Server-side correction** — **cited** `ServerGamePacketListenerImpl.handleMovePlayer`:
  the server does not simulate a player's movement; it teleports the body back
  when a move's squared length exceeds the client's by more than `100` (a larger
  allowance, `300`, under the one condition the handler reads beside it), or when
  the body ends inside a block it was not already touching by more than `0.0625`
  squared (`moved wrongly`). A movement the client's physics
  allows is never undone.

## 9. The rigs the numbers in §4–§5 were taken on

Flat stone, the body at rest in the middle of a cell, holding forward (and sprint)
toward `+x`, the jump pressed on the last tick the body is still `onGround` before
leaving the launch platform, the landing platform a declared number of air columns
away at a declared rise, a lid where stated. The scratch simulator and
prismarine-physics 1.11.1 both run every rig; the landing verdicts, apexes and
per-tick positions agree. The pinned jar's own code is reached by neither of them
without a running game; spec-0113 §6 states how the engine's simulator is held to
all three.

## 10. The ballistic measurement against the pinned jar

**Measured once, never re-checked** (a check guards only what our toolchain
owns). The instrument is `delvec::compiler::ballistics` — a PackTest of four
`minecraft:armor_stand` bodies thrown with a declared `Motion` over the rig of
`dsl::metrics::body::ballistics` (stone; slime under a four-block drop; a pool
four deep; a course of bottom slabs), each body's `Pos` read every tick for
sixty ticks as `data get entity … Pos[a] 10000` and asserted within `1e-4` of
the trajectory the engine's simulator computes under `Rules::Game`. It is
written by hand (`cargo test -p delvec --test movement_ballistics_instrument
-- --ignored`) and run with `validation/packtest-run.sh`; no build emits it and
no CI job runs it.

- **Revision**: engine `88328cfbd` (the simulator and instrument as measured);
  Minecraft Java 1.21.11, server jar sha256
  `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726`; PackTest
  2.4.0 in `ghcr.io/stellarfeline/delvewright-toolserver@sha256:31ed5d5a5c50158470bfee8c7197a75e6f586a25c950bf25b69b533f88e0ba0b`.
- **Result**: all 720 position asserts (4 bodies × 60 ticks × 3 axes) hold;
  the committed trajectories are
  `crates/dsl/tests/fixtures/movement/ballistics-1.21.11.json`.
- **What the measurement settled**, each a red before its change:
  - **In water** (read site `Entity.updateFluidHeightAndDoFluidPushing`): the
    body is in water when a water cell's surface is at or over the bottom of
    its box deflated by `0.001` (§6). The simulator had used prismarine's box
    (`contract(0.001, 0.401, 0.001)` against the box's top); the water body
    read y `303.7489` where that predicted `303.4718` on its first tick in the
    pool. `Rules::Game` now uses the jar's test, for water and lava;
    `Rules::Bot` keeps prismarine's.
  - **A mob with `NoAI` is not moved**: a `minecraft:zombie{NoAI:1b}` summoned
    with `Motion [0, 0, 0.6]` read z `0.5` on every tick. The instrument's
    bodies are armour stands, living bodies the game moves with no input.
  - **An armour stand does not step** (read site
    `ArmorStand.createAttributes`, `step_height 0`): slid into a slab's face
    it stopped with its front edge on the face (z `1.75`).
