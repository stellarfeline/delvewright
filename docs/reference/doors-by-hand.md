# A door a body opens by hand

What the pinned game does when a player in adventure mode uses a door, a trapdoor or a fence gate with an empty hand. Reader: the engine work that decides which blocks the nav model walks through and which a `barred` edge may be made of (spec-0123 §3.1). Established once against the pinned jar and treated as constant (CLAUDE.md, *A check guards only what our toolchain owns*); the read site is stated so the reading can be repeated when the pin moves.

## The read site

- The server jar `versions.toml [minecraft].server_jar_url`, sha256 `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726` (the pin), its inner `META-INF/versions/1.21.11/server-1.21.11.jar`.
- The ProGuard mappings piston-meta publishes for the version, `downloads.server_mappings`, sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`, which give every class and member below its Mojang name; no obfuscated name is written here.
- `javap -p -c` from a JDK 21, over the classes `BlockSetType`, `DoorBlock`, `TrapDoorBlock`, `FenceGateBlock`, `ServerPlayerGameMode`, `ServerGamePacketListenerImpl`, `ServerPlayer`, `Player`, `GameType` and `InteractionResult`. The method the maintenance tool `tools/maintenance/extract-peaceful-despawn.py` reads the same jar by.

## What the bytecode says

1. **`BlockSetType` carries `canOpenByHand`** as the second constructor argument, stored in its own field and returned by `canOpenByHand()`. In the static initialiser: `iron` is constructed with `canOpenByHand = false` (and `gold`, which no door uses); `copper`, `stone`, `polished_blackstone` and every wood type (`oak`, `cherry`, `crimson`, `warped`, `bamboo` and the rest, through the one-argument constructor whose three flags default to `true`) with `true`.
2. **`DoorBlock.useWithoutItem`** returns `InteractionResult.PASS` when `type.canOpenByHand()` is false, and otherwise cycles the `OPEN` property, sets the block with flags `10`, plays the open or close sound and emits the game event, returning `SUCCESS`. **`TrapDoorBlock.useWithoutItem`** has the same first branch: `PASS` unless `canOpenByHand()`, then the toggle. **`FenceGateBlock.useWithoutItem`** consults no `BlockSetType`: it toggles `OPEN` in every case (turning the gate to face the player when opening) and returns `SUCCESS`.
3. **`ServerPlayerGameMode.useItemOn`** (the server's handling of a use-on-block packet, called from `ServerGamePacketListenerImpl`'s handler after the position and reach checks) reads the game mode **once**: `SPECTATOR` opens a menu provider and returns. Every other mode — adventure included — calls `BlockState.useItemOn` with the held stack, and when that answers *try the empty hand*, `BlockState.useWithoutItem`, which is where the door, the trapdoor and the gate toggle. The field `GameType.ADVENTURE` is read nowhere in `ServerPlayerGameMode`, the packet handler, `ServerPlayer`, the three block classes or `GameType` itself except through `GameType.isBlockPlacingRestricted()` (true for `ADVENTURE` and `SPECTATOR`), whose one caller among them is `Player.blockActionRestricted` — the breaking and placing rule, not the use path.

## What follows, for this engine

- **An adventure-mode body opens by hand**: every wooden door and wooden trapdoor, the copper door and copper trapdoor, and every fence gate. It does **not** open an iron door or an iron trapdoor by hand; those move only by redstone.
- So in the nav model a door, trapdoor or gate block whose set type `canOpenByHand` is true is a cell a body passes — it opens the block and walks through, in either state the block stands in — and one whose set type is `iron` is a barrier in its closed state and a passage in its open state, exactly as a bar is.
- A `barred` edge's `bar.block` must therefore be a block a hand does not open: iron bars, an iron door, an iron trapdoor, a wall of any solid; a wooden door standing as a bar is a gate the player opens before the story does.
- An iron door as a bar is declared as its lower half (`half=lower`), with the program laying the lower half in the bar's lowest course and the upper half over it; the engine derives the same two states for the gate region from the one declared block (`compiler::region_fill`, `docs/reference/compiler.md`, *A region write of a two-high block*), and the story opens it as it opens any gate, by clearing the region.
- The set type of a door, trapdoor or gate block is read from the pinned block-state registry's block id (`*_door`, `*_trapdoor`, `*_fence_gate` by material), with `iron_door` and `iron_trapdoor` the two the hand does not open; the engine states the list it derived in its binding line and never writes it by hand.

## A leaf's shape

A door's and a trapdoor's collision box are one shape class, a leaf: a plate 3/16 thick standing in one cell, whose place depends on the state. What each one's box is, per state, and what follows for a body near one. Reader: the nav model's crossing rule (`blockshape::leaf`, `Leaf::stops_crossing`, `World::leaf_stopping`), `DW1066` and `DW1068`.

### A trapdoor's leaf

#### The read site

- The same jar and mappings as above.
- `javap -p -c` over `TrapDoorBlock`: its static initialiser builds `SHAPES` as `Shapes.rotateAll(Block.boxZ(16, 13, 16))`, a full-face box 3/16 deep; `getShape` returns `SHAPES.get(FACING)` when `OPEN` is true, and otherwise `SHAPES.get(DOWN)` for `half=top` and `SHAPES.get(UP)` for `half=bottom`. `useWithoutItem` never loads its `BlockHitResult` argument, and `ServerGamePacketListenerImpl.handleUseItemOn` reads the hit face only for the build-height message: no face of the block is refused.
- A second method sharing no configuration with the first: a probe that boots the registries and prints `BlockState.getCollisionShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO).toAabbs()` for every trapdoor state, and the committed tables `crates/dsl/data/collision-footprints-1.21.11.tsv` and `collision-tops-1.21.11.tsv`, which `blockshape::leaf` reads. 1344 states (21 ids × 64) collapse onto 16 shapes, 84 states each: `powered` and `waterlogged` move nothing, and every trapdoor id, iron and copper included, takes the same box for the same `facing`, `half` and `open`.

#### What the bytecode and the boxes say (cited)

| state | box, in sixteenths (x, y, z) |
|---|---|
| closed, `half=top` | `0,13,0 .. 16,16,16` — a plate across the top of the cell |
| closed, `half=bottom` | `0,0,0 .. 16,3,16` — a plate across the bottom |
| open, `facing=north` | `0,0,13 .. 16,16,16` — a panel against the south face |
| open, `facing=south` | `0,0,0 .. 16,16,3` — against the north face |
| open, `facing=east` | `0,0,0 .. 3,16,16` — against the west face |
| open, `facing=west` | `13,0,0 .. 16,16,16` — against the east face |

So the open panel stands against the face **opposite** `facing`, the whole cell high, whatever the half. `facing` names the side the trapdoor swings toward; its hinge is on the other side.

#### What follows for a body (authored, derived from the table and the player's 0.6 × 1.8 box)

- A body centred in a trapdoor's column clears an open panel on any face but the one it stands on: its box spans 0.2..0.8 of the cell and a side panel starts at 0.8125, so it clears by 1/80. Crossing the panel's own face, at any height that meets the cell, it is stopped.
- A body rising through an open trapdoor's cell is not stopped by it: the panel is vertical. What it stops is a step **across** its face while the body's box meets the cell.
- A closed top plate starts 13/16 up its cell. A body standing on the floor of the cell below fits under it with 1/80 to spare (1.8 against 1.8125), and steps up one block under it only to that height: a body whose feet would be in the trapdoor's own cell must open it first.
- A closed bottom plate is 3/16 high, under the 0.6 auto-step: a body walks over it.
- A body opens a trapdoor from below: nothing in the use path reads which face was hit.
- A hatch turned **across** a stair (its `facing` along the climb) stops the climb in both states: open, the panel of the trapdoor over the top tread stands on the face the climb crosses onto that tread; closed, its plate is where the body's head goes on the tread. A body in the cellar cannot come up, and a body on the top tread cannot go down. That is the nav model's crossing rule, `Leaf::stops_crossing`, and the trap `DW1066` names.

#### Measured on a running server

The Latch (content `demo/the-latch` a37ef92, engine `feature/a-place-reads-as-what-it-is` 3b36d3434) holds three oak trapdoors, `facing=north,half=top`, at x 18, y 63, z 15..17, over a stair climbing north: treads under z 17, 16 and 15 put the feet at y 61, 62 and 63, the last in the north trapdoor's own cell, with the room's floor at y 64 beside it. Its build was served on a compose project of its own and walked by a mineflayer 4.37.1 client (the harness's pin), the trapdoors set by right-click and by `/setblock`:

| hatch | from the cellar floor | from the room's floor |
|---|---|---|
| `facing=north`, closed | stopped on the z 16 tread, feet 62: the closed plate over z 15 | — |
| `facing=north`, open | stopped on the z 17 tread at z 17.300: the z 16 panel; shut again, up to z 16, then stopped at z 16.300 | in at the north hole onto the z 15 tread, then stopped at z 15.512 going down: the z 15 panel |
| `facing=south`, either | stopped on the z 17 or z 16 tread | — |
| `facing=east` or `west`, open | up onto the z 15 tread and off it onto the floor beside, east or west by the side the panel is not on | — |

Every stop is the body's box against a panel's face (a centre 0.3 from it) or its head against a plate's underside (feet 62.0125 under the plate at 63.8125), the figures the table gives.

### A door's leaf

#### The read site

- The same jar and mappings as above.
- `javap -p -c` over `DoorBlock`: its static initialiser builds `SHAPES` as `Shapes.rotateHorizontal(Block.boxZ(16, 13, 16))`, a full-height box 3/16 deep against the cell's south face for `north`, turned for the other three. `getShape` reads `FACING` and `OPEN` and, when `OPEN` is true, `HINGE`: shut, it returns `SHAPES.get(facing)`; open, `SHAPES.get(facing.getClockWise())` for `hinge=left` and `SHAPES.get(facing.getCounterClockWise())` for `hinge=right`. It never reads `HALF` or `POWERED`. `DoorBlock` declares no `getCollisionShape`, so the collision box is the shape.
- A second method sharing no configuration with the first: the committed tables `crates/dsl/data/collision-footprints-1.21.11.tsv` and `collision-tops-1.21.11.tsv`, measured by the registry probe, which `blockshape::leaf` reads. Every one of the 21 door ids, iron and copper included, in all 32 states of `facing`, `hinge`, `half` and `open`, takes the box the bytecode gives, full height in both halves (`blockshape::tests::a_door_reads_its_leaf_from_the_jar`, 672 states).

#### What the bytecode and the boxes say (cited)

| state | box, in sixteenths (x, y, z) |
|---|---|
| shut, `facing=north` | `0,0,13 .. 16,16,16` — a panel against the south face |
| shut, `facing=south` | `0,0,0 .. 16,16,3` — against the north face |
| shut, `facing=east` | `0,0,0 .. 3,16,16` — against the west face |
| shut, `facing=west` | `13,0,0 .. 16,16,16` — against the east face |
| open, `hinge=left` | the shut box of `facing` turned clockwise: `facing=north` stands against the west face |
| open, `hinge=right` | the shut box of `facing` turned counter-clockwise: `facing=north` stands against the east face |

So a door's leaf stands against the face opposite `facing` when shut, and against one of the two side faces when open, which one set by `hinge`. Shut and open, the leaf is on two different faces of its cell, and both halves take the same box.

#### What follows for a body (authored, derived from the table and the player's 0.6 × 1.8 box)

- A body centred in a door's column clears its leaf on any face but the one it stands on, by the same 1/80 as a trapdoor's panel; crossing the leaf's own face, it is stopped.
- A body that opens a door by hand sets it before it steps, and toggling it does not read the bodies in its cell (`useWithoutItem`, above). Its shut leaf and its open leaf stand on two faces, so for every face there is a state that clears it: a door a hand opens never stops a body that may use it. That is `Leaf::stops_crossing` with `may_use`, and why the party is never refused by one.
- A body that opens nothing — a mob, a `tp`-driven puppet — meets the door as it stands. A door standing open with its leaf across the way stops it: a door turned along a doorway (`facing` across the walls) swings its open leaf across the opening. The trap `DW1068` names; the remedy is a `facing` that sets the shut leaf in the wall's plane and its open leaf against a side of the opening.
- A body wider than 10/16 meets a leaf on every face of its cell, open or shut: a door's opening is 13/16 wide.

## A trapdoor over a stair

How a hatch is set over a stair so that a body going up the stair is never shut under it. Each rule says whether it is cited or authored.

- **The hinge is on a long side of the hole, so the open panel stands along the climb** (authored, derived from the shapes above and confirmed on the server). For a stair that climbs along z, the trapdoors take `facing=east` or `facing=west`; along x, `facing=north` or `facing=south`. Cited for the principle: the wiki's placement rule that a trapdoor's hinge sits on the block it was placed against and `facing` is the side opposite it (minecraft.wiki, *Trapdoor*, block states), and the Java ladder-hatch practice of a trapdoor over a ladder facing the same way as the ladder, its panel against the ladder's wall (minecraft.wiki, *Trapdoor*, "climbable"; `LivingEntity.trapdoorUsableAsLadder` in the pinned mappings). No published builder's guide for a hatch over a stair was found; the stair rule is this record's own.
- **Top half, flush with the floor** (cited, minecraft.wiki *Trapdoor*: a trapdoor placed on the top part of a block opens downward and lies flush with the floor above; *Tutorial: Trapdoor uses*, "Entrance": take out the block over the head and set a trapdoor there). A bottom-half hatch stands 3/16 proud of the floor, under the auto-step, and opens upward into the room.
- **It opens down into the stairwell, and stands against the side** (cited by the bytecode table: a top-half trapdoor's open panel spans its whole cell against one face). Nothing in the room's floor is in the way of a panel that hangs into the hole.
- **Headroom over the top tread** (authored, from the table). The top tread's feet are in the hatch's own cell, so the body must open the hatch over it to stand there; every lower tread keeps 1.8 blocks clear under a closed top-half hatch with 1/80 to spare, so a body walks the lower treads with the hatch shut.
- **Where the stair starts** (authored). The top tread lies under the hatch, with the room's floor beside it one block up on the side the panel does not stand on, so the body steps off sideways past the open panel. A stair whose top tread is one block under a hatch it cannot step off beside — floor only ahead of it, across the panel — is the trap again.
- **Unsupported**: no source was found for how far the stair should run under the floor before the hatch, or for more than one hatch in a row over one stair beyond what the shapes say; three in a row is legitimate when every one is turned along the climb.

Sources: minecraft.wiki, *Trapdoor* and *Tutorial: Trapdoor uses* (CC BY-NC-SA, ideas only: the facts were re-established against the pinned jar where they bear on the engine). The craft gap — no established builder's practice for a hatch over a stair — is recorded against this page.
