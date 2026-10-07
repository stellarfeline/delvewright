# spec-0092: A lightning bolt strikes at a mark

- **Status**: Proposed
- **Ground**: written against engine `51dfc4773` (`origin/main`), read only — the stage-5 effect vocabulary (`Verb`, 43 variants, `crates/dsl/src/stages.rs`), `emit::emit_firework` and its proof `compiler::firework`, `compiler::lethal::posted_places`, the setup gamerules in `emit.rs`, and the pinned 1.21.11 game itself: `net.minecraft.world.entity.LightningBolt`, `Entity.thunderHit`, `ServerLevel.canSpreadFireAround` and `ChunkMap.anyPlayerCloseEnoughTo`, disassembled with `javap` from the pinned client jar and named through Mojang's official 1.21.11 client mappings. The dedicated server's own `LightningBolt` class (`META-INF/versions/1.21.11/server-1.21.11.jar` inside the pinned server jar) is byte-identical to the client's (SHA-256 `c96d262fe294ef4e…` for both), so every fact below is the server's.
- **What it is for**: a storm that strikes where the story says. A thunderstorm is already a world state (`set-weather thunder`), but the bolt a scene needs — the strike that lights a colossal thing for one heartbeat, the bolt that splits the tree the party sheltered under — falls where the game rolls it, or nowhere. The first scene to ask is the owner's cutscene on the demo level **The Thing Beyond the Fog**: the camera stops, a bolt strikes beside the figure in the sea, and in that same tick the fog is torn away.
- **Research**: every game fact is read from the pinned bytes and is marked **cited** with the method it was read by; the rules built on them are **authored**.
- **Numbers**: `spec-0092` is the only number taken here. **Three DW codes**: `DW0958` and `DW0959` (§5), and `DW0960` (§10). The effect vocabulary gains a verb inside the unpublished `dsl_version` the unreleased format changes share; this spec states no version literal.
- **Non-goals**: a bolt that is only a picture (the game's `visualOnly` flag is not stored by the entity's save data, so no `summon` can set it — §2); a bolt aimed at a body; natural lightning (a `thunder` world rolls its own and this spec does not touch it); fire (the engine's sealed gamerule already prevents any, §2.3); a storm of many bolts as one verb — that is a `sequence` of strikes, which the vocabulary already writes.

## 1. The defect

**Finding, from reading the tree.** `delvec schema --stage all` at the ground revision carries the word `lightning` only as a damage type (`DamageKind::LightningBolt`, `minecraft:lightning_bolt`, which `damage-players` deals without a bolt). No effect summons one. The nearest members of the class a strike would join are `firework` (spec-0068) and `particle` (spec-0085 §4.3): a one-shot thing that happens at a mark. A creator who wants a bolt today has a `play-sound` of `entity.lightning_bolt.thunder` and a particle, which give neither the bolt, nor the sky flash every client draws for it, nor the light.

**What the same demo level found already in the engine** (recorded so nobody writes a surface for it):

- *A brief fog cut* is two `set-atmosphere` repaints in a `sequence` (spec-0080). On the client a repaint is a cut, not a fade: the demo level's measurement (its README, *The fog flash, measured*) reads it through the client's own classes.
- *A statue built for one side* is a form whose box ends behind the body (spec-0087): the box's back face is the flat back, and the sculpt needs nothing else.
- *Glowing eyes* are hand lights of the form (`lights[].at` with an emissive block), set into cut sockets; the palette refusal of emissive tones does not reach them.

## 2. What the game does with a bolt

### 2.1 The entity

**Cited — `LightningBolt` bytecode.** `minecraft:lightning_bolt` is summonable and present in the pinned entity registry (`crates/delvec/data/entities-1.21.11.json`). On its first tick (`life == 2`) the server spawns fire (§2.3), powers a lightning rod in the struck block, scrapes a weathering or waxed copper block it strikes (§2.4) and emits the `lightning_strike` game event; every client in tracking range plays `entity.lightning_bolt.thunder` at volume 10000 (heard everywhere) and `entity.lightning_bolt.impact` at volume 2, and sets the sky flash for as long as the bolt lives. The bolt lives three ticks and re-flashes a random one to three times; its save data reads and writes nothing, so `visualOnly` (the trident-free spelling of a harmless bolt) cannot be set by `summon`.

The **struck block** is `BlockPos.containing(x, y − 1.0E-6, z)`: for a bolt summoned on a mark's plane, the block under the mark.

### 2.2 What it hits

**Cited — `LightningBolt.tick` and `Entity.thunderHit`.** While it lives, the bolt calls `thunderHit` on every living entity whose box meets `[x − 3, y − 3, z − 3] .. [x + 3, y + 9, z + 3]` (the constants 3.0 and 6.0 + 3.0). The base `thunderHit` sets the entity alight for eight seconds and deals 5 HP of `minecraft:lightning_bolt`. Eight classes override it (`MushroomCow`, `CopperGolem`, `Pig`, `Turtle`, `ArmorStand`, `BlockAttachedEntity`, `Creeper`, `Villager`): among them a villager becomes a witch, a pig a zombified piglin and a creeper is charged. A spectator is invulnerable to it. A player standing in reach takes at most 5 + 8 = 13 HP — under a full body's twenty.

### 2.3 Fire

**Cited — `LightningBolt.spawnFire` → `ServerLevel.canSpreadFireAround` → `ChunkMap.anyPlayerCloseEnoughTo`.** The bolt places fire only when `canSpreadFireAround(pos)` holds, which reads `fire_spread_radius_around_player` and asks whether a non-spectator player stands **strictly closer** than that radius. Every delve's setup writes `gamerule fire_spread_radius_around_player 0` (`emit.rs`), and no distance is less than zero: a bolt in a delve lights nothing. That holds only while the gamerule line does, so the line is asserted by the test that lands the verb (§8.5).

### 2.4 Copper and rods

**Cited — `LightningBolt.clearCopperOnLightningStrike`, `powerLightningRod`.** When the struck block is a lightning rod the bolt powers it; when it is a weathering or waxed copper block the bolt de-oxidises it and walks a **random** path through neighbouring copper scraping more. A world write the game rolls is a write no proof of this engine can state.

## 3. The surface

**Authored.**

```json
{ "type": "lightning", "at": { "anchor": "anchor/node-near", "offset": [63, -1, -171] } }
```

- **`at`** — a [`Mark`] (spec-0066), the one field. The bolt stands at the cell's centre, on the mark's own plane, so it strikes the block under the mark.
- One effect, one bolt. A storm is a `sequence` of strikes.
- It is a **party fact**: the emitter addresses no player (the thunder is the game's to send), so `audience` and `in` are refused on it (`DW0942`), as on `firework`.

## 4. Emission

**Authored, on §2's cited parts.** One line: `summon minecraft:lightning_bolt <x+0.5> <y> <z+0.5>`, absolute, walked against the pinned command tree like every line the emitter writes.

## 5. The refusals

**Authored.** `compiler::lightning`, build tier (exit 3), judged over the assembled world once the mark is a cell — `compiler::firework`'s neighbour.

- **`DW0958` — a bolt in reach of a posted body.** Any place the campaign requires a body to be — `DW0511`'s own enumeration (`compiler::lethal::posted_places`): the entry, every checkpoint and bonfire seat, every NPC and actor post, every cast placement, every wave seat — whose body meets the bolt's box of §2.2: the hitbox the enumeration states, standing at its cell's centre, overlapping the box strictly on every axis, as `AABB.intersects` tests. The message names the bolt, the post and what the bolt does to a body; the remedy is the mark or the post.
- **`DW0959` — a bolt that rewrites a block.** The struck block is a lightning rod, or a copper block the game weathers (§2.4). The message names the block and why; the remedy is the mark, or a block the bolt leaves alone.

**What it deliberately does not catch, stated rather than implied.** Players are not posted, as for `firework`: a player standing within three blocks of a strike takes at most 13 HP and is set alight, and it is a hazard the creator places in plain sight of the beat. A cutscene puts every player in spectator (`dw_cutscene`), where the bolt cannot hurt them.

Every build that assembles a world prints `lightning binding: L strike(s) declared, S struck block(s) read, P post(s) within reach examined, R refused` — zeroes included — and a campaign that declares one writes `validation/lightning-gate.json`.

## 6. What the gallery, the record and the skill owe

**Authored.**

- **The gallery element.** The valley-site overlay's walled court, which already ends on a firework, strikes a bolt at its far end, clear of every post; bound by perturbation — moving the offset moves the `summon` line, and moving it three cells toward the arrival is `DW0958`. Units: `QuestEffect::lightning` and its `at`.
- **The probes.** `a-bolt-beside-the-arrival` (`DW0958`): a strike two cells from the hall's arrival, where every player first stands. `a-bolt-on-a-copper-tile` (`DW0959`): a waxed copper tile laid under the exit, and a strike on it.
- **The PackTest template.** `lightning_<n>` runs the effect's line and asserts a `minecraft:lightning_bolt` stands at the mark on the same tick.
- **The record.** `docs/reference/compiler.md`: the surface row, the emission row beside `firework`'s, the two codes and the binding line.
- **The skill.** `references/quest-capabilities.md` under *Things that change the world*.
- **The demo level.** **The Thing Beyond the Fog**, `docs/demo-levels.md`.

## 7. Scope

A bolt aimed at a moving body, a charged-creeper trap, and a lightning rod as a gameplay device are each a different verb with a different proof. Natural lightning under `thunder` is the game's and is not modelled.

## 8. Acceptance criteria

`delvec` is this tree's binary, built from the crate manifests this tree carries.

1. **The surface.** `delvec schema --stage all` exports `QuestEffect::lightning` with `at` a `Mark` and nothing else; the effect union names 45 verbs (44 with this spec's verb; spec-0094's `arm-strikes`, assembled in the same release, makes 45).
2. **The emission.** A campaign declaring one strike emits exactly one `summon minecraft:lightning_bolt` line at the mark's cell centre and plane, walked against the pinned command tree; two builds are byte-identical.
3. **The reach.** A player's body posted one cell inside each face of §5's box is `DW0958` naming the post; one cell outside each face is green; the keeper of the hello-world hall, a villager, refuses a strike on his stand end to end; the posts are `DW0511`'s own enumeration.
4. **The struck block.** A strike over a waxed copper block or a lightning rod is `DW0959`; the same strike over stone is green.
5. **No fire.** A campaign declaring a strike emits `gamerule fire_spread_radius_around_player 0` in its setup.
6. **The party fact.** `audience` or `in` on a `lightning` effect is `DW0942`.
7. **The binding line.** Every build that assembles a world prints §5's line; the valley-site overlay reports `L = 1` and the primary `L = 0`.
8. **The gallery.** §6's element builds green; both probes are refused with their codes; `tools/ci/check-gallery-coverage.py` reports 0 units in neither state.
9. **The live half.** The demo level's PackTest runs `lightning_<n>` green on the pinned server.
10. **The record and the skill.** The rows and paragraph of §6, in the pull request that lands the code; `tools/ci/check-dw-codes.py` green.
11. A demo-level row names **The Thing Beyond the Fog**.

## 9. Decisions for the owner

- A strike is **one effect, `lightning`, at a mark**, and a real bolt: it lights the sky on every client, it thunders, and it can hurt what stands within three blocks — the alternative, a harmless bolt, does not exist in the pinned game's `summon`.
- A strike **beside a posted body, or onto a block it would rewrite, is refused** (two new codes) — the alternative is an advisory, which would ship a villager that turns into a witch on the beat.
- Players near a strike are **not proved safe**, as for `firework`, and the record says so.
- The verb lands in the unpublished `dsl_version`; no ADR.

## 10. What the owner's walk found: the carry, the bound and the free camera

**Cited — the owner's walk of the demo level, reproduced on the pinned server** (`demos/the-thing-beyond-the-fog/measure/repro.mjs` in the content repository: a mineflayer client that pulls the tiller through its interaction entity). From the ferry's well one cell forward of the stern, the first pull played the cutscene and left the client on the near jetty; the second printed the trigger's marker and did nothing. The carry volume was the stern row only; the client stood in the boat outside it; and `cs_end` puts every player on the cell the presser stood on, so the carry took nobody. The route proof stood the one cell it chose inside the volume, and so did the bot. The far jetty lay inside the boundary's region, and a client carried from the stern row with the boundary clock running arrived at the far jetty: the bound was not the cause. Three defects of the engine stand behind it, and one requirement of the owner's.

**Authored.**

1. **A carry after a cutscene takes everyone or no one** (`DW0932`, a fifth fault, "pressed from outside its volume"). For a link whose root plays a `cutscene` that ends before its teleport, every cell of the walk region that performs the trigger — by the rule the route proof's stand cells are read by — must lie inside `from`. The trigger's own cell may not lie in the volume (`DW0542`), so it must be a cell no body stands in.
2. **The boundary clock skips a player watching a cutscene and a creator out of the body.** `boundary_tick` selects `@a[tag=!dw_cutscene,tag=!dw_free]`: a cutscene is pure observation (compiler.md, *A cutscene is pure observation*), and the creator overlay's free camera (spec-0069) is a creator tool that a player bound must not fight. One PackTest per tag.
3. **`boundary.returns`** (default `true`): `false` keeps the region — every proof that reads it reads the same box — and emits no clock. It is the creator's switch for a world nobody can leave; the owner's demo levels take it, so a far view looked at with the free camera is never cut off.
4. **`DW0960` — the boundary and the world agree.** `returns: false` is refused where a reachable walkable cell lies outside the region or the walk region enters the open sea; and every place a body is put — a critical-path step's cell, a link's or a gather's landing — must lie inside a region that returns. The second shape cannot fire on a document that validates today (every such cell is an anchor in a placed piece, `DW0897`, and the region holds every piece); it is stated so the region and the places are checked against each other rather than assumed to agree.

**Acceptance criteria, §10.**

12. A link carried after its root's cutscene, whose trigger a body can perform from a cell outside its volume, is `DW0932` naming the cells. *Gallery probe `a-tiller-pulled-from-the-hall-floor`; `nav::tests::a_press_from_outside_the_volume_is_found`; the demo level's build at content `f88a21a` refused with 34 cells, its rebuild green with 0.*
13. The boundary clock skips `dw_cutscene` and `dw_free`; `returns: false` emits no clock and keeps the region. *`crates/delvec/tests/v36_boundary.rs`; the `boundary_exempt_*` PackTests.*
14. `returns: false` over a world a body can leave is `DW0960`; a place outside a region that returns is `DW0960`. *`crates/delvec/tests/v36_boundary.rs`; the gallery's ocean overlay with `returns: false` refused, naming the quay cell a body walks into the open sea from.*
15. The gallery binds `boundary.returns` (the void-horizon overlay, `false`).
