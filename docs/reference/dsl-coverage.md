# DSL coverage — what ten campaign concepts need, against what `delvec` can say

Reader: the agent planning engine work. The question is where a creator with a fresh idea hits "vanilla can, the engine cannot". Ten deliberately different concepts were pitched (setting, tone, scale, core verb, danger, party shape, hour all varied, and all unlike the souls-castle line); this file maps every vanilla mechanic they depend on to the engine as it is.

**Instrument.** `delvec 1.7.1, dsl 0.35.0, mc 1.21.11`, release build of engine revision `1ef29efa` (`cargo build --release -p delvec`, exit 0), surface read from `delvec schema --stage all` (542 968 bytes; quests stage 105 definitions, 38 `QuestEffect` verbs, 5 objective kinds, 4 trigger `on` kinds, 3 horizon bases). Rows below cite the schema type, `docs/reference/compiler.md`, the gallery (`gallery/quests.json`), or the spec that records the gap. Vanilla facts cite the Minecraft Wiki; anything not measured on the pinned server says so.

**Verdicts.** **S** supported: a declared surface emits it and the proofs read it. **P** partial: expressible by composition, or emitted without a proof that judges it, or one half missing. **U** unsupported: no surface, or the surface refuses it.

## 1. Mechanics

| # | Mechanic | Verdict | Evidence |
|---|---|---|---|
| M01 | A scene hung high over an un-walkable floor (sky over a cloud sea) | P | `HorizonBase` = `void`/`ocean`/`valley`; spec-0026 §4 `sky` base (backdrop, `float_y`, `fall`) is Proposed and unbuilt. `void` hangs pieces in nothing and `boundary` (floor = lowest placed block − 8) returns a faller to the checkpoint, so the shape exists without a visible floor below. |
| M02 | Per-zone sky: cloud height, sky/fog colour, ambient particles, music, sun angle | U | No biome or environment-attribute surface; `/fillbiome` is emitted only for a `valley` surround band (compiler.md "The horizon's surround"). Vanilla: 1.21.11 moved these to environment attributes definable on a biome, a dimension type or a timeline, resolved at the camera — `visual/cloud_height`, `visual/sky_color`, `visual/fog_*`, `visual/ambient_particles`, `audio/background_music`, `visual/sun_angle` ([Environment attribute](https://minecraft.wiki/w/Environment_attribute); biome `attributes` since 25w42a, [Biome definition](https://minecraft.wiki/w/Biome_definition_(Java_Edition))). Unmeasured: whether `/fillbiome` reaches a connected client without a chunk reload. |
| M03 | A level that rises or falls by story stage | P | Solid planes: `fill-region` / `clear-region` are emitted and modelled (`plan::RegionEvent`; gallery binds both). Water: spec-0038 is Accepted but has no surface — its own probe P3 ("no field") still holds; a runtime water fill is impassable and never standable (`DW0544`). |
| M04 | A hazard volume that exists only from a story stage | P | `LethalVolume` = `{id, region, message, damage_type, shown_by}`, no gate, always live. `damage-players{in}` is a one-shot; `timed_gates[].crush` is clocked, not staged. |
| M05 | Vertical climbing (ladder, vine, scaffolding) on a required route | U | Nav steps are cardinal, one cell up or down (compiler.md "Nav"); a climbable is a no-collision fixture, not a move. |
| M06 | Jumping across a gap | U | Same nav rule: no non-adjacent move. |
| M07 | A long drop as a route | P | `layout-graph` edge class `drop` exists; the voxel nav admits −1 only. |
| M08 | Riding a mount or vehicle (minecart, happy ghast, horse, boat) | U | spec-0038 §5 refuses vehicles; nav has no ridden body. `equipment.body`/`saddle` dress a mount (spec-0067) but nothing rides it. Vanilla: a harnessed happy ghast carries four players in free flight ([Happy Ghast](https://minecraft.wiki/w/Happy_Ghast)). |
| M09 | Player-operated redstone as an input (lever, button, plate) | P | `traps[].trigger` reads `pressure-plate`/`tripwire`/`trapped-chest`; env `triggers[].on` = `strike`/`use`/`approach`/`strike-npc` on interaction hitboxes. A lever's block state is never read. |
| M10 | Branching dialogue and branching endings | S | Stage 6 option gates; `branch_points[]` (spec-0025); `campaign-complete.ending`. Gallery binds `branch_points`. |
| M11 | A crowd of background speakers | S | `cast` ledger + bark pools (spec-0020); gallery binds `cast`. |
| M12 | NPCs relocating as the story moves | S | `move-npc`, per-quest `cast` placement, `spawn-npc`/`despawn-npc`. Story-driven only; nothing is keyed to clock time. |
| M13 | Hour and weather changing with the story | S | `set-time` (six states), `set-weather`; cut, cycle frozen. |
| M14 | Readable documents (letters, diaries, written books) | P | `give-item`/`loot` carry `name` and `enchantments` only; kit `lore` is an unknown field (`DW0100`); no `written_book_content`. An `interact` + `narrate` shows a line. |
| M15 | Knowledge only the player who looked has | S | `state[].scope: player`; a `presser`-audience trigger writes it; dialogue options and cast placements evaluate per player (compiler.md "Which sites can touch a per-player datum"). Gallery binds `scope: player` and `audience: presser`. Flags stay party-wide. |
| M16 | Fireworks | S | `firework` effect (spec-0068, `DW0899`); gallery binds it in `gallery/overlays/valley-site/quests.json` and refuses one under a roof (`gallery/probes/a-rocket-under-a-roof`). |
| M17 | Player body attributes (`scale`, `gravity`, `jump_strength`, `step_height`, `safe_fall_distance`) | U | No player-attribute surface: kit `attributes` is undefined, `give-effect` is status effects only, `MobAttributes` has four mob fields. Nav assumes one player body. Vanilla: all apply to players, by `/attribute` or an item's `attribute_modifiers` (`scale` 0.0625–16, `gravity` −1–1; [Attribute](https://minecraft.wiki/w/Attribute)). |
| M19 | Swimming as a route, with a breath budget | U | Nav refuses fluid as floor or passage; spec-0038 §5 refuses wading/swim traversal. |
| M20 | Status effects | S | `give-effect` / `clear-effect` (spec-0031); gallery binds both. |
| M21 | Hostile waves, ambushes, bosses with a health bar | S | `waves[]`, `ambushes[]`, `actors[]` + `unleash-actor`, `health_bar` (spec-0016, 0073). |
| M23 | A threat that hunts by sound (warden, sculk) | P | A warden wave emits; a sculk sensor's output is not an input (see M09) and no proof says a quiet route exists. |
| M24 | A companion that follows the party | U | `move-npc`/`move-actor` go to a mark; no follow target. |
| M25 | Moving quietly past a listener | P | Vanilla sneaking already damps vibrations; `begin-stealth` is zone presence only, by design (no sneak requirement); nothing proves the route. |
| M26 | The party in two copies of one place, carried between them | S | Multiple `areas[]` + `teleport{from,to}`; gallery binds `teleport`. |
| M27 | Several players acting at the same moment (two plates held at once) | U | Every gate term latches (`requires_flags`, `requires_state`); no trigger reads "these N volumes are occupied now". |
| M28 | An act in one place changes another | S | `set-block` / `fill-region` / `open-way` at any anchor from any root. |
| M29 | Crafting, cooking, brewing as the verb | P | Vanilla crafting works in adventure mode ([Adventure](https://minecraft.wiki/w/Adventure)); the DSL has no recipe surface, and `collect` needs a container at an anchor. A product is checkable when presented (`interact.requires_item`, held). |
| M30 | Snowfall in one area | U | Weather is dimension-global; snow needs a cold biome, which is not authorable (M02). |
| M31 | A deadline that fails and rearms | P | Composable: `sequence` steps gated by `when.forbids_flags`, a sidebar datum counted down by `add-state`, `clear-state`/`close-gate` to rearm. No first-class clock and no proof that the critical path fits inside it. `timed_gates[]` (S) cover cycling doors. |
| M32 | Content only one class can read or use | P | Only through items: `carrier: one`, `interact.requires_item` held. The gate has no class term. |
| M33 | Collapses and traps | S | `collapse`, `volley`, `traps[]` (spec-0011, 0022); gallery binds all three. |
| M34 | Breaking a chosen block with a tool (archaeology brush, `can_break`) | U | No item-component surface at any item site. Vanilla adventure mode breaks only with `can_break` and places only with `can_place_on` ([Adventure](https://minecraft.wiki/w/Adventure)). |
| M36 | Per-player score on screen | S | `state[].display: sidebar` (spec-0076); gallery binds `display`. |
| M40 | Vanilla sound cues | S | `play-sound` (registry-checked, `DW0326`). Custom audio is not served. |
| M46 | Handing an item to a character | S | `interact.requires_item` (held), `shops[]`. |
| M47 | Combination puzzles | S | `state` + `requires_state` comparisons, `DW0879` path replay. |
| M48 | A creature that patrols a house and chases | P | Hostile species chase natively (waves); `lane` routing is Raider-only (spec-0016 §6). A species vanilla never makes hostile cannot be made to hunt (see §4). |
| M49 | Enemies in water | P | Drowned on land emit; `locomotion: aquatic` is refused (`DW0455`) and water is never floor. |
| M50 | Sparse respawn points | S | `set-checkpoint`, `bonfire`. |

## 2. Per concept

Score = S ÷ total; the weighted column counts P as one half.

| Concept | Mechanics | S | Weighted |
|---|---|---|---|
| 1 Whale-fall: climb a dead sky whale's skeleton over a rising cloud sea | M01 M02 M03 M04 M05 M08 | 0/6 = 0% | 25% |
| 2 Lantern-festival murder: night-long town detective mystery | M10 M11 M12 M14 M15 M16 | 5/6 = 83% | 92% |
| 3 An inch tall: shrunk players cross a cottage | M17 M05 M06 M48 M20 | 1/5 = 20% | 30% |
| 4 Ebb city: sluice gates lower a drowned city floor by floor | M03 M19 M20 M27 M09 M49 | 1/6 = 17% | 42% |
| 5 Last mountain train: minecart run with switch points | M08 M09 M13 M12 M21 | 3/5 = 60% | 70% |
| 6 Fifty years apart: half the party in the past, half in the present | M26 M28 M15 M27 M02 | 3/5 = 60% | 60% |
| 7 Snowbound inn: one night of cooking for strange guests | M29 M30 M46 M12 M11 | 3/5 = 60% | 70% |
| 8 Silent mine: escort a blind miner past a warden | M23 M24 M25 M20 M40 M50 | 3/6 = 50% | 67% |
| 9 Tomb before the sandstorm: 20-minute class-split heist | M31 M32 M27 M33 M34 M02 | 1/6 = 17% | 33% |
| 10 Academy finals: five rule-changing exam rooms, scored | M17 M20 M36 M31 M29 M47 | 3/6 = 50% | 67% |

The supported half is story, people and fights; the unsupported half is **the player's body and the world around it** — how a body moves, what size it is, what it rides, what sky it stands under.

## 3. The fewest general mechanisms, ranked by concepts unlocked

| Rank | Mechanism (object class it belongs to) | Closes | Concepts | Sketch |
|---|---|---|---|---|
| 1 | **Level-sensitive conditions** (the trigger) | M27, M09, M23 input | 4, 5, 6, 8, 9 | A trigger `on` a condition evaluated now rather than latched: N declared volumes occupied at once, or a declared block state at an anchor (lever powered, plate pressed, sculk sensor active); the path replay credits it like any trigger step. |
| 2 | **Zone atmosphere** (the area) | M02, M30 | 1, 6, 7, 9 | An area or region declares environment attributes; the compiler emits a datapack biome carrying them and paints it with `/fillbiome`. Also answers precipitation (biome temperature) and `gameplay/monsters_burn`. |
| 3 | **Player movement repertoire** (the player body, nav) | M05, M06, M07, M19 | 1, 3, 4 | One table of player moves measured against vanilla physics — climb a climbable column, clear a gap of n, drop n with fall damage counted, swim a fluid body with a breath budget — read by every route proof. |
| 4 | **Item stack = vanilla component map** (the item, at every item site) | M14, M34, gear half of M17 | 2, 9 (3, 10 with rank 5) | One item type across kit, `give-item`, `loot`, shop offers and drops carrying `written_book_content`, `lore`, `can_break`, `can_place_on`, `attribute_modifiers`, each registry-checked. |
| 5 | **Player attributes** (the player body) | M17 | 3, 10 | A gated, scoped attribute modifier verb (and item modifiers via rank 4); nav reads the body's scale, jump and gravity, so it needs rank 3. |
| 6 | **Gated region declarations** (the region) | M03 water half, M04 | 1, 4 | Lethal volumes and other region declarations carry the shared `Guard`; spec-0038's flood level lands as specified. |
| 7 | **Rides** (a body that carries bodies) | M08 | 1, 5 | A mount or vehicle as a body class with passengers and a proven route (rails for a minecart, free flight for a happy ghast). |
| 8 | **Clocks** (runtime state) | M31 | 9, 10 | A periodic effect root that moves a datum, a trigger on a gate becoming true, and a path-time bound that proves the critical path beats the clock. |
| 9 | **Obtain as an objective** (the objective) | M29 | 7, 10 | `collect` completed by acquiring an item by any means, container optional, so a crafted or brewed product counts. |
| 10 | **Follow** (the body) | M24 | 8 | A body follows a player only through vanilla follow AI (tamed wolf or cat owner, allay); other species are refused (§4). |

Ranks 1–3 alone lift the mean weighted coverage from 56% to 78%.

## 4. Refuse permanently (no-hacks rule)

- **Moving block structures** (a sinking skeleton or a train built of blocks that travels): vanilla has no moving block assembly. The vanilla answers are region writes, entities (minecarts) and teleport.
- **Handheld dynamic light**: no vanilla primitive; moving light blocks after a player is a hack.
- **Per-area day time**: `/time` is per dimension. A zone's apparent hour is spelled through rank 2 (`visual/sun_angle`, `visual/sky_color`), never per-player time flicker.
- **Follow or hunting AI for species vanilla does not give it** (per-tick teleports or move loops).
- **A runtime LLM** for interrogation-style mysteries: already forbidden; dialogue is authored branches.

## 5. Findings

- `gameplay/monsters_burn` is a per-biome vanilla attribute in 1.21.11. The daylight-burn findings (`hv-08`, `isl-17`, `isl-40`) are answered today by refusal (`DW0496`); rank 2 would let a creator state the rule instead.
- Asymmetric knowledge already works: `scope: player` state plus per-player dialogue and cast gates; only flags are party-wide.
