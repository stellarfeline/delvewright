# spec-0099: A body climbs — the nav model holds a body on a ladder or a vine the world keeps, every proof that moves a body counts the climb, the waypoints carry it, and the bot drives it

- **Status**: Proposed
- **Ground**: written against engine `d44b3a5d4` (`origin/main`). Read: `crates/dsl/src/blockshape.rs` (`collision_class`, `is_no_collision_plant`, `is_no_collision_fixture`), `crates/dsl/src/metrics.rs`, `crates/delvec/src/compiler/assembled.rs` (`occupancy_over`), `crates/delvec/src/compiler/nav/world/{mod,body}.rs` (`neighbors_fp`, `moves_of`, `settle_fp`, `fatal_step_off`), `nav/route/{mod,leg}.rs` (`judge_leg`, `route_with_links`, `verify_exported_routes`), `nav/leave.rs`, `compiler/waypoints.rs`, `harness/src/{waypoints.ts,executor/walk.ts}`, mineflayer-pathfinder 2.4.5 (`index.js`, `lib/movements.js`) and prismarine-physics 1.11.1 (`index.js`) as the harness lockfile pins them.
- **What it is for**: a level whose vertical links are climbed — the rope ladders between the treehouses of The Treehouse Camp (`docs/demo-levels.md`), a ladder out of a well, vines down a cliff — proven completable by machine. On `d44b3a5d4` a ladder is a full cube to the nav model, a vine is air, nothing climbs, and a route that needs a climb is refused (`DW0311`) or cannot be proven.
- **Research**: §2 is this spec's record. Each statement is **cited** (the pinned server jar read by mapped name, its data files, a vendored table, or a line of a pinned dependency) or **authored** (this spec chooses).
- **Numbers**: spec `0099`. One diagnostic: `DW0991` (§5.2). No DSL surface change: climbing is the nav model's reading of vanilla blocks a piece already places, so no `dsl_version` is owed.
- **Non-goals**: a layout-graph or site-plan edge class for a climb (the treehouse seams' `form` is spec-0098's surface; a piece that lays a ladder is proven by this spec whatever drew it); scaffolding as a climb (§3.3); the open-trapdoor-over-ladder rule (§3.4); a mob's pathfinding onto a climbable (§4.4 states what the model assumes instead).

## 1. The thing, and the object it belongs to

**Authored.** Climbing is a fact about **a block state under the pinned game** — which blocks a body climbs, what holds each where it is — and a fact about **the body's movement** over the assembled world. So:

- *Which blocks are climbed, what holds them, which of their faces are full* belongs to `delvewright_dsl::blockshape`, beside the collision table, read from the pinned jar.
- *Which climbables the assembled world keeps* belongs to the occupancy model (`compiler::assembled::climb_holds`), which holds the block map.
- *How a body climbs* belongs to the one movement relation in `compiler::nav` (`World::climb_moves_fp`), taken by the route relation, by the body and mob relations, and by the fall — never by a verb or a proof privately.

## 2. What vanilla does (cited)

All bytecode citations are of the pinned server jar (`versions.toml` `server_jar_sha256` `f83b8e09…1726`, inner jar sha256 `ec47239a…fcebc`), read with `javap -c` and the official mappings (sha1 `5621e925…5955`) by mapped name.

### 2.1 Which blocks are climbed

`data/minecraft/tags/block/climbable.json` in the jar: `ladder`, `vine`, `scaffolding`, `weeping_vines`, `weeping_vines_plant`, `twisting_vines`, `twisting_vines_plant`, `cave_vines`, `cave_vines_plant`. `LivingEntity.onClimbable()`: false for a spectator; otherwise reads `getInBlockState()` — the block at `blockPosition()`, **the feet cell** — and answers true when it is in `#climbable`, or when it is an open trapdoor `trapdoorUsableAsLadder` accepts.

### 2.2 How a body moves on one

- `LivingEntity.handleRelativeFrictionAndCalculateMovement`: when `(horizontalCollision || jumping) && onClimbable()`, the movement's `y` is set to `0.2`. A body climbs by pushing into something **or by holding jump**.
- `LivingEntity.travelInAir`: `vy' = (vy − gravity) · 0.98`, gravity the `0.08` attribute; so the climb moves `(0.2 − 0.08) · 0.98 = 0.1176` blocks a tick, 2.35 a second (`metrics::climb_blocks_per_tick`).
- `LivingEntity.handleOnClimbable`: on a climbable it calls `resetFallDistance()`, clamps `x` and `z` to `±0.15` and `y` to at least `−0.15`; when `y < 0`, the block is not `SCAFFOLDING`, `isSuppressingSlidingDownLadder()` and the body is a `Player`, `y` is set to 0. `Player.isSuppressingSlidingDownLadder()` returns `isShiftKeyDown()`. A player who sneaks holds still on every climbable but scaffolding; one who does nothing slides at most 0.15 a tick; a fall that reaches a climbable is forgotten.

### 2.3 What holds each

- `LadderBlock.canSurvive`: the block at `pos.relative(facing.getOpposite())` answers `isFaceSturdy(level, pos, facing)` (`canAttachTo`). Its shape is `Block.boxZ(16, 13, 16)` rotated to the facing: a 3/16 panel against the block behind it.
- `BlockStateBase.isFaceSturdy(level, pos, dir)` is the four-argument form with `SupportType.FULL`, whose `isSupporting` is `Block.isFaceFull(getBlockSupportShape(…), dir)`.
- `VineBlock.canSurvive` = `hasFaces(getUpdatedState(…))`. A horizontal face set true is kept by `canSupportAtFace`: the neighbour across it is acceptable to `MultifaceBlock.canAttachTo` — `isFaceFull` of its **support** shape **or** of its **collision** shape, on the face toward the vine — or the block above is a vine carrying the same face. The `up` face is re-read as `isAcceptableNeighbour(level, pos.above(), Direction.DOWN)`.
- `GrowingPlantBlock.canSurvive`: the block at `pos.relative(growthDirection.getOpposite())` is the plant's head or body, or answers `isFaceSturdy(…, growthDirection)`. The growth directions handed the constructors: `DOWN` for weeping and cave vines (head and body), `UP` for twisting vines. Only kelp overrides `canAttachTo`.
- `FlowingFluid.canHoldAnyFluid` lists `LADDER` among the blocks no fluid enters; a vine is not listed.

### 2.4 Collision and light

`crates/dsl/data/collision-tops-1.21.11.tsv`: `ladder` 0..16 (its panel), every vine `-` (empty). `minecraft-data` 1.21.9 `blocks.json` (the vendored table `compiler::light` already cites): `filterLight` 0 for `ladder`, `vine`, `weeping_vines`, `twisting_vines`, `cave_vines`.

### 2.5 The bot

- mineflayer-pathfinder 2.4.5, `lib/movements.js`: `this.climbables` holds `ladder` (`vine` is commented out); `getMoveUp` moves a node one up when its block is climbable; `index.js` `monitorMovement` steers with `bot.look(Math.atan2(-dx, -dz), 0)` and calls a node reached at `|dx| ≤ 0.35 && |dz| ≤ 0.35 && |dy| < 1`.
- prismarine-physics 1.11.1, `index.js` `isOnLadder`: `ladder` or `vine` (and the 1.9–1.20 trapdoor rule); climbs on `isCollidedHorizontally` or, with `climbUsingJump` (listed for 1.21), on jump.

## 3. Scope

### 3.1 The climbables (authored, from §2.1)

Every member of `#climbable` but scaffolding is `Collision::Climbable`: a body passes the cell and stands on nothing there. The ladder's 3/16 panel leaves a centred 0.6-wide body clear of it (§2.3). A ladder dams a flood, a vine does not (§2.3), and a flooded cell holds no climb.

### 3.2 What the world keeps (authored rule over cited holds)

A climbable is a climb only while its hold (§2.3) holds, read over the assembled block map **to the fixed point** the game's shape updates cascade to: the game's own check reads one neighbour, and a chain whose top hangs on nothing falls a block at a time once an update reaches it. A climbable whose hold fails is read as air and recorded, with its block, for `DW0991`. Faces are read from `crates/dsl/data/faces-1.21.11.tsv`, measured by `tools/maintenance/dump-faces.py` (`sturdy` = `isFaceSturdy`, `full` = `isFaceFull` of the collision shape). A vine's `up` face is not counted as a hold — it can only drop a vine the game keeps. A runtime write that touches a climbable's own cell or any cell its hold reads takes the climb away, whatever block it lays: the derived world carries cells, not block states, so it credits no relaid face.

### 3.3 Scaffolding (authored exclusion)

Scaffolding stays the full cube its stable shape is. Its collision depends on where the body is and whether it sneaks (`ScaffoldingBlock.getCollisionShape` reads the `CollisionContext`), so one cell is floor to a body above it and passage to a body inside it; a body cannot hold on it (§2.2: sneaking descends); and its `distance` ties a cell to up to seven others. The nav model's cell classes cannot state a block that is both, and modelling it as a cube refuses climbs, never invents one.

### 3.4 The open trapdoor over a ladder (authored exclusion)

Not modelled: trapdoors are full cubes to the nav model, so no body is ever put in a trapdoor's cell. A trapdoor over a ladder is a hatch, and its rule waits on trapdoor collision being modelled at all.

### 3.5 The catch (authored rule over cited physics)

A climbable stops a fall only on a tick that begins with the feet in its cell (§2.1). A body moving less than one block a tick cannot pass a one-block cell between two ticks, so a climb cell catches a fall whose drop from the body's feet to the cell's top is at most `metrics::climb_catch_fall_blocks()` = 7 — derived from the fall law of §2.2 (the 14th tick moves 0.966 having fallen 7.56; the 15th moves 1.025). A body that does not hold on slides to the bottom of the run and falls on from there with its fall forgotten.

## 4. The model

### 4.1 Holding (authored)

A **climb cell** for a body: its feet cell holds a climbable the world keeps, every cell of its body is unoccupied, and no killing volume reaches it (`World::climb_cell_fp`). It need not be standable. Every route cell is standable or a climb cell (`World::holds_body`).

### 4.2 The climb's moves (authored, each a thing a body does on cue)

From a climb cell: up, down, off the side onto a standable cell level with it or one lower, over the top from the top climb cell onto a standable cell one higher beside it (the column above holds the risen body: the body clears the top rung by at least 0.15, §2.2), and off the bottom onto a standable cell under a run that stands on nothing. From a standable cell: into a climb cell beside it, or one lower when the cell over that is clear (a step off the brink the climbable catches within a block). A body holding in mid-air walks nowhere. These are the only climb moves, and the route relation takes them (`World::neighbors_fp`). Step cost: the shaped cost every route already uses, one flat step plus the elevation weight per sixteenth — a rung costs three flat steps.

### 4.3 Where a body can end up (authored)

`World::body_moves` and `World::mob_moves` take the climb's moves, a step off the side into a fall, letting go at the bottom of a run, and the catch (§3.5) on every fall; a body holding in mid-air does not jump. `World::fatal_step_off` reads the catch and the slide: a step into a column that a climbable catches is fatal only if the fall below the run is.

### 4.4 Mobs (authored)

A wave member climbs as the player does. Vanilla's ground pathfinder plans no climb, but a mob pushed into a ladder climbs it (§2.2 reads `LivingEntity`); every proof that asks where a mob can get (`DW0922`, `DW0923`, `DW0924`) asks the wider question, which can only refuse more.

## 5. The proofs

### 5.1 What reads the model

Every proof that moves a body reads the relations of §4 and nothing else: the critical and branch route proofs (`DW0311` and family), the pacing walk (`DW0822`), the leave proof (`DW0921`), the blinding reach (`DW0943`), the wave reaches (`DW0922`/`DW0923`/`DW0924`), the stage-5 portal crossing (`DW0986`, `nav::World::neighbors`), and the exported-route check (`DW0314`, which admits a climb cell).

### 5.2 `DW0991` — a forced leg climbs a climbable the world does not keep

Derived by counterfactual, like `DW0510`/`DW0317`/`DW0544`/`DW0546`: a leg that does not route is re-routed with every unheld climbable credited as if it hung (each held by its own cell, so a write that overwrites it still removes it), walking and then through the live links; if that routes, the leg is refused naming each unheld climbable on the route and the hold it lacks. Build tier (exit 3).

### 5.3 The waypoints

Each exported leg carries `climbs[]` — `from` (where the body takes hold), `to` (where it lets go), `bottom` and `top` (the column it holds in), `block`, and a ladder's `facing` — emitted only when present. A climb's `from` and `to` are kept waypoints; the cells held between them are not waypoints, because a waypoint is a place to stand.

## 6. The harness (authored, over §2.5)

A hop whose ends are a climb's `from` and `to` is driven by the climb executor (`harness/src/executor/climb.ts`), never by the pathfinder: into the column's centre, then push toward the block the ladder hangs on (a vine: toward the column) holding jump until the feet clear the let-go floor, or — going down — hold nothing and slide to the let-go height; then step onto the let-go cell; bounded by a budget of six seconds plus one a block. mineflayer-pathfinder is used unchanged for every other hop. A climb on a block prismarine-physics does not climb — a weeping, twisting or cave vine — is refused by name: the compiler proves it, and the bot cannot walk it (a recorded harness gap, not a route defect). A reach a carry's landing completes consumes its exported leg, so the walks after a carry stay in lockstep with the legs.

## 7. The gallery's obligation, and the demo level

The gallery hall's ferry cabin gains a ladder up its east wall (`CABIN_LADDER`, four rungs facing east), the only way onto its roof, and `obj/climb-onto-the-cabin` (anchor `anchor/cabin-roof`) on the critical path between crossing the strait and reaching the end, so the route climbs up and back down. `gallery/probes/a-ladder-with-nothing-behind-it` turns the four rungs to face west, into the wall, and is refused `DW0991` by `build`. The Treehouse Camp (`docs/demo-levels.md`, spec-0098's row) is where the capability is confirmed: its rope-ladder seams are pieces that lay ladders.

## 8. Decisions

1. **The climbables are the tag's, minus scaffolding** (§3.1, §3.3). A creator gets every climbable vanilla has but scaffolding, whose exclusion is stated.
2. **A climbable is a climb only while vanilla would keep it**, to the fixed point, from measured faces (§3.2).
3. **One movement relation** (§4): the climb is moves in `nav`, taken by every relation; no proof has a climb of its own.
4. **The catch is derived, not chosen** (§3.5).
5. **One new refusal**, `DW0991`, a counterfactual; an unheld climbable that no forced route needs is not refused.
6. **The bot drives a climb hop itself**, and refuses a climbable its physics cannot climb by name (§6).
7. **No DSL surface**: no `dsl_version`.

## Acceptance criteria

1. `crates/dsl/data/faces-1.21.11.tsv` exists, written by `tools/maintenance/dump-faces.py` from the pinned jar (header names the jar sha256 and the mappings sha1); `blockshape::tests::the_face_table_reads_the_jar` asserts stone sturdy on all six faces, leaves full and never sturdy, a bottom slab sturdy only down, and an id the pin lacks sturdy nowhere. *Vacuous if* the table were empty or unread: the leaves assertion (full, not sturdy) cannot pass on a missing row, which answers `false` to both.
2. `blockshape::CLIMBABLE_1_21_11` holds the nine ids of the jar's `#climbable`; `blockshape::tests::the_climbable_class_is_the_pinned_tag` asserts every member but scaffolding is `Collision::Climbable` (passes a body, never a floor), scaffolding is `FullCube`, and glow lichen, chain and hanging roots are not climbed. *Vacuous if* the list were not the tag: the test asserts its length is 9.
3. `nav::world::climb::tests::a_body_climbs_a_ladder_that_hangs`: up a four-course face no step or jump reaches, a route exists with a ladder facing away from its support, holding in every rung and stepping over the top; none exists with no ladder; none with the ladder turned to face its support (`climb_census` `(0, 4)`), and one exists over `with_unheld_climbs`. *Vacuous if* the route used a step instead of the ladder: the bare face has no route.
4. `leaves_hold_a_vine_and_not_a_ladder`, `a_hanging_chain_is_kept_from_its_top` (weeping vines kept under a ceiling, none kept without it; vine faces held by the vine above) and `clearing_the_support_drops_the_climb` (a runtime clear behind one rung drops it and the route) pass. *Vacuous if* holds were not read: the leaves ladder and the ceilingless chain would be kept.
5. `a_ladder_catches_a_fall_and_the_body_slides_on`: `climb_catch_fall_blocks()` is 7; a step off a ledge five blocks over a ladder run is caught at its top rung (`body_moves`); `fatal_step_off` is fatal when the run hangs over void and `None` when a floor is under it. *Vacuous if* the catch were not read: the floored case would report the fall past the run as fatal.
6. `a_ladder_is_a_way_out_of_a_pit` (`nav::leave`): a four-deep shaft is `DW0921` with nothing to climb and passes with a ladder on its wall, the fall in caught at the top rung. *Vacuous if* the leave proof did not take climbs: the laddered shaft would stay `DW0921`.
7. `a_leg_up_a_ladder_is_proven_and_an_unheld_ladder_is_dw0991`: a forced leg up a hanging ladder routes; up a ladder facing its support it is `DW0991`, naming `[2, 66, 1]` and "sturdy east face"; with no ladder it is `DW0311`. *Vacuous if* the counterfactual named nothing: the message must carry the cell and the face.
8. On the gallery primary point built by this branch: the leg to `anchor/cabin-roof` is proven (`DW0311 binding` counts it) and `validation/critical-path-waypoints.json` carries one climb on that leg (up, `from` `[5, 67, 28]`, `to` `[4, 71, 28]`) and one on the leg back (down). The same documents and pieces built by `d44b3a5d4`'s `delvec` exit 3 with `DW0311` on the leg to `[2, 71, 28]`. *Vacuous if* the roof were reachable without the ladder: the base build would route it.
9. `gallery/probes/a-ladder-with-nothing-behind-it` is refused by `delvec build` with `DW0991`, naming the four rungs `[5, 67..70, 28]` and the sturdy west face each lacks. *Vacuous if* the probe perturbed nothing the proof reads: it changes only the rungs' facing.
10. `harness/test/waypoints.test.ts` parses the cabin legs' `climbs`, marks exactly the `from → to` hop as the climb, refuses a climb whose ends are not its leg's waypoints or whose column is skewed, and `clientClimbs` refuses `weeping_vines_plant`; `harness/test/executor.test.ts` asserts a reach the landing completes advances the leg cursor. *Vacuous if* no hop carried the climb: the test asserts exactly one does.
11. The CI `gallery bot` job passes on this branch with a `[climb] … 1 climb(s) on the proven leg, 1 driven as a climb hop` line for each of the two cabin legs and `[climb] … let go at` for each. *Vacuous if* the walks after the carry matched no proven leg (the pathfinder then climbs on its own and the job passes regardless): the binding line names the driven count.
12. `docs/reference/compiler.md` and `docs/reference/delvec/compiler/nav.md` carry `DW0991`; `docs/reference/compiler.md` describes the climb model and the `climbs` export; `docs/reference/playtest-methodology.md` describes the climb hop and its binding line; `docs/reference/tools.md` lists `dump-faces.py`; the gallery README names the cabin ladder and the probe.
