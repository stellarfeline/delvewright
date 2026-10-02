# spec-0082: A fixed thing that can be hit and hits back — the animated display-entity assembly

- **Status**: Proposed
- **Ground**: written against engine `e3a6dd36` (`origin/main`), read only, with
  the instrument `delvec 1.7.1, dsl 0.35.0, mc 1.21.11` built from that tree
  (`cargo build --release -p delvec`, exit 0): `delvec schema --stage all` is
  542 968 bytes, its quests stage carries 105 definitions, 38 `QuestEffect`
  verbs and 4 `TriggerOn` kinds, and the words `assembly`, `clip` and `display`
  name nothing a creator can declare (every occurrence is jigsaw assembly, a
  camera clipping a wall, or the engine's own `item_display` hardware). Read
  with it: `Actor`, `Prop`, `EnvTrigger`, `TriggerOn`, `Verb::DamagePlayers`,
  `Verb::Cutscene`, `StealthZone`, `Mark`, `LethalVolume`, `StateDecl` in
  `crates/dsl/src/stages.rs`; `compiler::pressable::body_at`,
  `compiler::eclipse`, `compiler::strand::STRIKE_REACH`,
  `compiler::camera`'s spike measurements, `dsl::metrics::keep_out_box`;
  `docs/reference/compiler.md`'s trigger, `damage-players`, actor and hardware
  rows (`DW0420`, `DW0421`, `DW0426`, `DW0427`); the design record of the
  campaign that first needs this (content repository, branch
  `campaign/stranding` at `a3e5953e`, its mechanism paragraph and coverage
  table — no story is repeated here); the research spike
  `research/eldritch-visuals` at `2bbb1f28` (`tools/spike-eldritch-visuals/`,
  a 34-segment tentacle of 68 block displays over 96 keyframes); and this
  spec's own rig, `tools/spike-display-assembly/`, run on the pinned server
  (§8, every number with its instrument).
- **What it is for**: a thing that stands in the world, is built of display
  entities, moves through authored clips, can be struck by a player until it
  gives way, and strikes back at a player who stands where it can reach — a
  limb from a pit, a statue that swings, a door of teeth, a pendulum of chains.
  The campaign that commissions it needs several at once and needs them as
  optional enemies; the mechanism is general and names none of that.
- **Research**: §9 is this spec's research record. Every rule below is marked
  **cited** (a vanilla behaviour measured on the pinned server, a page named,
  or a constitution rule) or **authored** (this spec chooses).
- **Numbers**: spec `0082`. DW codes are **placeholders** here
  (`DW_ASSEMBLY_*`, §6) and are allocated by the planner at implementation. No
  ADR: no settled decision moves. **`dsl_version` moves by one minor step**:
  the quests stage gains an object class, three verbs and a trigger kind.
- **Non-goals**: a thing that walks (step 2, §7, planned and not specified
  here); a hitbox that is a living body (§3.4 says why, §11 asks); health,
  equipment, traversal, a health bar or a kill credit on an assembly — it is
  not a fight class and never dies; a clip authored by hand in campaign JSON
  (§3.1: a clip is a procedural derivation and lives in a rig file the tool
  writes); an `on_end` hook on a clip (a `sequence` already times the beat
  after a clip, and the compiler knows every clip's length); text displays;
  item displays as parts (named as the next part kind in §3.1, not written);
  sound on a strike (an ordinary `play-sound` in the landing bundle); a
  cutscene that follows an assembly.

## 1. The finding

The primitive exists and is cheap; the engine has no surface for it. The
research spike stood two 34-segment tentacles of `minecraft:block_display`
entities and advanced them through 96 precomputed keyframes with
`start_interpolation` / `interpolation_duration`, and the pinned server ticked
at 5.6 ms average with both animating against 7.0 ms before they woke (its
`observations.json`, 100-tick samples). The engine already summons
`minecraft:interaction` hitboxes and reads their `attack` record for every
`strike` and `strike-npc` trigger, already pairs an invisible hitbox with an
`item_display` for every affordance it owns (`DW0420`, `DW0421`), already
deals damage inside an anchor-centred box (`damage-players{in}`), and already
counts things in a declared datum behind an ordinary gate (`state[]`,
`add-state`, `requires_state`). Nothing joins them: there is no object a
campaign can name that is made of display entities, no clip, no way to count
hits on a thing that is not a character, and no strike.

The coverage record of the commissioning campaign states the same gap as its
capability **A′**, planned in two steps, and needs step 1 only.

## 2. The object class — a new one, and why

**Authored**, against `CLAUDE.md`'s *This is a general engine* paragraph: a
capability belongs to the object class it acts on, and a second bespoke field
is the defect. Three existing classes were read before a fourth was opened.

- **An `actors[]` entry** is a vanilla mob body: NoAI puppet, `Invulnerable`,
  `equipment`, `attributes`, `traversal`, `health_bar`, `on_kill`, `unleash`
  into a real-AI twin. Every one of those is a fact about a living body with
  a vanilla hitbox and vanilla AI. An assembly has none: display entities *do
  not move, do not take damage, do not make sounds, and have no collision*
  [cited — Minecraft Wiki, *Display*]. Adding `parts`/`clips` to `Actor`
  would make eleven fields meaningless on one shape of the class.
- **A `Prop`** is one block id at an anchor. An assembly is many entities at a
  mark with a timeline; nothing of a prop carries over but the anchor.
- **A stage-2 `Npc`** owns the one thing an assembly shares — an
  `interaction` hitbox a left-click reaches — and nothing else (dialogue,
  persona, skin, a walk).

So: **`assemblies[]` is a new stage-5 object class.** What it *shares* it
takes from the classes that own those things, by type and never by copy:

| capability | owner it takes it from | how |
|---|---|---|
| where it stands | `Mark` (spec-0066) | `at: {anchor, offset?}` |
| a region (trigger, strike) | `StealthZone`, the engine's one anchor-centred box, resolved through `Plan::zone_box` | `strikes.while_in`, `damage-players.in` |
| being struck | the trigger class, `TriggerOn` | a new kind `strike-assembly {assembly}`, the exact shape of `strike-npc {npc}` (no `at`; rides the object's own hitbox) |
| counting hits and acting at a count | `state[]` + `add-state` + `Guard.requires_state` | written by the creator, no `hit_count` field anywhere |
| hurting a player | `damage-players{amount, in, damage_type}` | the landing bundle is an ordinary effect list |
| timing after a clip | `sequence` | no `on_end` |
| appearing and leaving | the `spawn-*` / `despawn-*` verb pair | `spawn-assembly`, `despawn-assembly` |

The one genuinely new thing is the **rig** — parts and clips — and that is
not a campaign-stage object at all (§3.1).

## 3. The surface

**Authored.**

### 3.1 The rig: a library artefact, not campaign JSON

A clip is thousands of numbers (the spike's tentacle: 34 parts × 96 frames ×
a 4-vector, a 4-vector and a 3-vector). `CLAUDE.md`: *every input to a
surface is either a creative judgement or a procedural derivation, handed by
the tool, never typed*. Keyframes are the second kind. They live in a **rig
file** beside the prefab library, generated by a deterministic program the
way `prefabs/*-generator` writes `.nbt`, and referenced from the campaign by
id exactly as a prefab is:

```
rigs/<name>/rig.json
{
  "rig_version": 1,
  "parts": [ { "id": "seg-0", "kind": "block", "block": "minecraft:sculk" }, … ],
  "clips": {
    "idle":    { "ticks_per_frame": 5, "loop": true,  "frames": [ [ <transform per part> ], … ] },
    "windup":  { "ticks_per_frame": 5, "loop": false, "frames": [ … ] },
    "strike":  { "ticks_per_frame": 5, "loop": false, "frames": [ … ] },
    "retract": { "ticks_per_frame": 5, "loop": false, "frames": [ … ] }
  },
  "provenance": { … }
}
```

- `parts[]` — one display entity each. `kind` is `block` (a `block_display`
  with `block_state`) today; `item` is the next kind and is not written. The
  block id is checked against the pinned block registry (the `DW0193` rule).
- `clips` — named; each frame is one transform per part, in the **rig's own
  frame**: origin at the assembly's mark (the cell's centre at the mark's
  plane), `+z` the rig's front. A transform is the display entity's
  `{translation, left_rotation, scale, right_rotation}` [cited — *Display*,
  entity data]. A non-looping clip holds its last frame. `ticks_per_frame` is
  the keyframe cadence and the `interpolation_duration` the compiler writes
  (§4.2); 1..=20.
- `provenance` — the generator and its revision (ADR-0013's record, as a
  prefab's metadata carries it).

A rig is checked and described by one creator-facing command, **`delvec rig
describe <rig>`**: part count, every clip with its length in ticks, and per
clip the **footprint of its last frame** — the cells the parts occupy, as
the compiler computes them from the transforms (§5.2). That footprint is the
number a creator hands to the strike region, so the region is declared from a
derivation the tool printed, never estimated from a picture.

### 3.2 The assembly

```json
{ "id": "assembly/pit-limb",
  "rig": "rig/tentacle-34",
  "at": { "anchor": "anchor/pit", "offset": [0, 0, 0] },
  "facing": "north",
  "initial": "idle",
  "hitbox": { "width": 3, "height": 8, "offset": [0, 0, 0] },
  "strikes": {
    "while_in": { "anchor": "anchor/pit", "extent": 10 },
    "pattern": [
      { "windup": "windup", "hold": 20, "strike": "strike",
        "on_land": [ { "type": "damage-players", "amount": 6,
                       "in": { "anchor": "anchor/pit-mouth", "extent": 2 } },
                     { "type": "play-sound", "sound": "entity.warden.sonic_boom",
                       "at": { "anchor": "anchor/pit-mouth" } } ] } ] } }
```

- `id` — `assembly/<kebab>`.
- `rig` — `rig/<name>`, resolved against the library like `prefab/<name>`.
- `at` — a `Mark`. The rig's origin. Held inside its piece by `DW0897`
  exactly as a body's mark is.
- `facing` — the four cardinals (`Facing`), default `south`. **Applied by the
  compiler to every frame** — translation rotated about the mark, rotation
  composed with the yaw quaternion — so the emitted entities stand at yaw 0
  and no client fact about how a display's own yaw composes with its
  transformation is relied on. Step 2 is where that fact is measured (§7).
- `initial` — the clip playing from spawn; `none` spawns the parts at the
  first frame of no clip, i.e. the rig's declared rest pose (`parts[].rest`
  if the rig carries one, else the first frame of its first clip).
- `hitbox` — optional. A `minecraft:interaction` of `width × height` whose
  bottom centre is the mark plus `offset`. Absent, the assembly cannot be
  struck and `strike-assembly` on it is refused. Bounds in §6.
- `strikes` — optional. `while_in` is a `StealthZone`; the pattern runs,
  repeating from its first step, on every tick on which some player's body is
  in that box, and stops at the end of the step in flight when nobody is.
  Each step: `windup` (a clip name), `hold` (ticks, the pose held after the
  windup's last frame), `strike` (a clip name), `on_land` (an ordinary effect
  list, run with the party audience on the tick the strike clip's **last
  frame is applied**). A step with no `on_land` is a feint and is legal.

### 3.3 The verbs and the trigger kind

- `spawn-assembly {assembly}` — idempotent, like `spawn-actor`.
- `despawn-assembly {assembly}` — the parts and the hitbox leave unseen; a
  display entity has no death, so there is no `style`.
- `play-clip {assembly, clip}` — switches the clip; the first frame is applied
  on the next tick. While a strike step is in flight the switch waits for the
  step to end, so a story beat never cuts a strike at the frame before it
  lands and leaves the damage unfired or the pose impossible.
- `triggers[].on: strike-assembly {assembly}` — no `at`, exactly `strike-npc`'s
  rule (`DW0194` on a mismatch); fires on the hitbox's `attack` record as
  `strike` does; `once: false` is how a counter is built; `audience: presser`
  is `DW0427` as it is for every left-click.

**A hit count is written, not declared.** The campaign writes
`state/limb-hits` (party scope), the trigger adds one per strike, and the
retract is an effect behind the ordinary gate:

```json
{ "id": "trigger/limb-struck", "on": "strike-assembly", "assembly": "assembly/pit-limb",
  "once": false, "forbids_flags": ["flag/limb-retracted"],
  "effects": [
    { "type": "add-state", "state": "state/limb-hits", "amount": 1 },
    { "type": "play-clip", "assembly": "assembly/pit-limb", "clip": "retract",
      "when": { "requires_state": [ { "state": "state/limb-hits", "op": ">=", "value": 5 } ] } },
    { "type": "set-flag", "flag": "flag/limb-retracted",
      "when": { "requires_state": [ { "state": "state/limb-hits", "op": ">=", "value": 5 } ] } } ] }
```

Effects in one bundle run in order, so the gated effects read the datum the
`add-state` just moved (the emission is one function, one line per effect,
each `execute if score …`). The flag stands the trigger down. Nothing here
is new machinery; that is the point.

### 3.4 Why the hitbox is an `interaction`, and what that costs

**Cited — this spec's rig, §8 rows 4 and 5.** A melee attack on the
`interaction` writes `attack: {player, timestamp}` and the poll counts it; the
hitbox may ride a display part and still register. **An arrow does not
register**: a bow shot and two summoned arrows passed through the box and
wrote nothing. A thing whose hitbox is an `interaction` is
therefore struck in melee only, and the creator's page says so (§10). The
alternative — an invisible living body as the hitbox, so `player_hurt_entity`
would credit arrows — is a *fight class* with health, knockback and a death,
and is the thing §2 declined to make an assembly into; §11 puts the question
to the owner once, with that cost.

## 4. Emission

**Authored, on measured facts (§8).**

### 4.1 Bodies

- One **root**, `minecraft:item_display` with no item (renders nothing), at
  the mark, tagged `dw_asm_<id>` and `dw_fixture` (its position is engine
  state — the `Teleport` verb's own exemption class, so a region teleport
  never carries an assembly away).
- One `minecraft:block_display` per part, tagged `dw_asm_<id>`,
  `dw_asm_<id>_p<i>`, summoned at the mark with the initial frame's transform
  and `interpolation_duration: <ticks_per_frame>`.
- **Every part rides the root** (a star): 34 `ride … mount` commands on one
  `item_display`, 34 passengers counted back (§8 row 2), a zombie holding
  two the same way. The star is what makes the assembly one thing to move
  (§7): a `tp` of the root moves every passenger with it and turns each by
  the root's own change of yaw (§8 row 3), and no part depends on another
  part's seat — a passenger that is itself teleported leaves its vehicle (§8
  row 3), so a chain would be broken by any one such write and a star is not.
- The hitbox, when declared: one `minecraft:interaction` tagged
  `dw_asm_<id>_hit`, standing on its own at the mark (not riding; step 2
  replaces it with the mob's body).

### 4.2 Clips

One function per clip frame (`asm_<id>_<clip>_f<n>`: one `data merge entity
@s {start_interpolation:0, transformation:{…}}` per part, addressed `as
@e[tag=dw_asm_<id>_p<i>,limit=1]`), one per-assembly driver on the tick
(`asm_<id>_tick`: a tick counter against the clip's cadence, a frame counter
with the clip's loop or hold rule, one macro dispatch `$function
<ns>:asm_<id>_$(clip)_f$(frame)`). This is the spike's own emission shape and
the cost rows of §8 are measured on it. `teleport_duration` is not used:
nothing here teleports.

### 4.3 Hits and strikes

- The trigger kind rides the existing trigger tick: `execute as
  @e[tag=dw_asm_<id>_hit,nbt={attack:{}}] run function <ns>:trig_<trigger>`,
  then `data remove entity @s attack` — the `strike` emission, retargeted.
- The strike pattern is a per-assembly state machine on the tick, exactly the
  spike's: idle → windup playing → hold counting → strike playing → land. The
  landing runs `on_land` through the ordinary effect emitter, so
  `damage-players{in}` is emitted as it is everywhere (`execute as
  @a[<box>,tag=!dw_cutscene] run damage @s <amount> <type>`), and a player in
  a cutscene is never struck (§4 of the record, *a cutscene is pure
  observation*).

## 5. What is checked

**Authored**, each rule naming the instrument it reuses.

### 5.1 The rig

Part count, block ids, every clip non-empty with one transform per part per
frame, `ticks_per_frame` in bounds, a finite transform (no NaN, a scale that
is not zero on any axis). Refused at validation (`DW_ASSEMBLY_RIG`).

### 5.2 Footprints — what the compiler computes from a frame

For a part's transform, the unit cube's eight corners under scale, rotation
and translation are the part's box; its cell set is the cells that box
intersects. A **frame footprint** is the union over parts; a **clip
footprint** is the union over frames. Pure arithmetic over the rig's numbers,
deterministic, and the same function `delvec rig describe` prints from.

### 5.3 The hitbox

- **It is marked.** The hitbox's box intersects the initial clip's first
  frame footprint — the player hits what the player sees — else
  `DW_ASSEMBLY_HITBOX`. The `DW0420` rule, on this hardware.
- **Its reach is real.** Vanilla detects attacks on an `interaction` only
  within 3.3 blocks of its position toward −X/−Z, 19.3 toward +X/+Z and 22.6
  above it [cited — Minecraft Wiki, *Interaction*, *Usage*]. A `width` over
  6 or a `height` over 22 has a slab nothing can hit, and is refused by the
  same code. (The spike's 3 × 8 box is inside both bounds.)
- **It contests no other affordance.** The assembly's hitbox enters
  `compiler::eclipse`'s enumeration, so `DW0359` and `DW0878` are asked of it
  as they are of every press body.
- **It can be struck from somewhere the party can stand.** When a
  `strike-assembly` trigger is one the critical path performs (it opens a way
  or sets a flag a later step reads — the rule the harness's `trigger` step
  already states), some standable cell of the party's population lies within
  `compiler::strand::STRIKE_REACH` (3.0 blocks, the attribute's default) of
  the hitbox's box, eye-to-box as `strand` measures it. Else
  `DW_ASSEMBLY_REACH`, the `DW0426` class: a beat that can never happen.

### 5.4 The strike — danger is visible, or the engine refuses it

The operating rule and spec-0062 say a killing volume never reaches a cell
that reads as safe floor. A strike is not a volume: it is a blow a player
watches coming. What makes its danger visible is three things, and each is a
check (`DW_ASSEMBLY_STRIKE`, one code, three shapes, build tier):

1. **It lands only where it was announced.** Every `damage-players.in` box in
   an `on_land`, grown to its keep-out (`metrics::keep_out_box(Body::PLAYER,
   …)`, the set of feet-cells a body can be caught from), lies inside the
   keep-out of `while_in`. A player who never entered the arming region is
   never struck. (The two regions are the same object class, so this is a box
   containment over two `Plan::zone_box` results.)
2. **The blow is where the limb is.** Every caught cell of a landing box
   (keep-out ∩ the standable population, spec-0062's own `P`) has a cell of
   the strike clip's **last-frame footprint** (§5.2) in its column, at or
   above its floor and no more than 3 cells above it. The thing the player
   saw come down is the thing that hurt them; a box the limb never reaches is
   refused naming the cells.
3. **The telegraph is long enough to read.** The windup clip's duration in
   ticks (frames × cadence) plus `hold` is at least **10 ticks** (500 ms).
   Authored from the cited reaction figures in §9: the average reaction to a
   single clear stimulus is about 265 ms, and an animation whose early frames
   barely move cannot be reacted to at all; the floor is set at roughly
   double the average, applied to the whole telegraph, and the demo level is
   where it is felt.

Not checked, stated rather than implied: how much a strike hurts. `amount` is
the creator's; a full body is 20; a strike of 20 or more kills an unhurt
player who stood in it. §11 asks whether that is refused, advised or allowed.

### 5.5 Clip references and the plan

A `play-clip`, `initial`, `windup` or `strike` naming a clip the rig lacks is
`DW_ASSEMBLY_RIG` at validation, the message listing the rig's clips. A
`strike-assembly` on an assembly with no `hitbox` is `DW_ASSEMBLY_HITBOX`. An
assembly's `at` is a `DW0360` site like every anchor-bearing declaration and
a `DW0897` site like every mark.

### 5.6 The binding line and the stated cost

Every build prints `assembly binding: A assembl(ies) declared, P part(s), C
clip(s), H hitbox(es) examined, S strike step(s) checked, R refused` — zeroes
included, so a green on a campaign with none is read as one — followed by the
cost the host meets: the total part count and the keyframe writes per tick
the declared cadences add up to, beside §8 row 7's measured rate. A host is
never a cap on the capability; the engine states the number.

## 6. Placeholder codes

`DW_ASSEMBLY_RIG` (validation, exit 1), `DW_ASSEMBLY_HITBOX` (build, exit 3),
`DW_ASSEMBLY_REACH` (build, exit 3), `DW_ASSEMBLY_STRIKE` (build, exit 3).
Four codes, allocated by the planner across every remote ref at
implementation; each owes a red fixture and its row in
`docs/reference/compiler.md` (`check-dw-codes.py`).

## 7. Step 2, planned: the assembly worn by a mob

**Not specified here.** Step 2 puts the assembly on an invisible vanilla mob
as passengers, syncs its facing to the mob and switches its clip from the
mob's state (walk, attack, idle), so an enemy can wear a silhouette vanilla
does not have while pathing, attacking, damage and the hitbox stay the mob's.
What step 1 leaves open so step 2 is an addition and not a rewrite:

- **One carrier.** The parts hang off one root (§4.1). Step 2 mounts that
  root on the mob — one `ride` (a mob holds several passengers, §8 row 2) —
  and nothing about the parts moves. Had step 1 summoned the parts
  free-standing, step 2 would re-emit every one.
- **Facing is the root's.** Step 1 bakes `facing` into the frames (§3.2) so
  that step 1 needs no client fact. Step 2 needs the parts to turn with the
  mob at runtime, and §8 row 3 draws the line it has to work inside: a
  vehicle **teleported** with a yaw turns every passenger by the same change,
  `/rotate` on the vehicle rotates the vehicle alone, and a passenger
  teleported in place **dismounts**. So a mob turning under its own AI does
  not turn the parts, and the per-part rotation write is the destructive
  one. What is left to step 2 is to measure whether a display's own yaw
  rotates its rendered transformation at all (a client fact this rig cannot
  see — the first thing step 2's demo level looks at) and, if it does, which
  vanilla write carries the mob's yaw to the root without unseating it. The
  frames being authored in the rig's own frame, with `facing` a separate
  step, is what leaves every route open.
- **The clip source is a slot.** Step 1's clip is set by the story
  (`play-clip`) and by the strike machine. Step 2 adds a third writer keyed
  to the mob's state; the driver reads one `clip` score per assembly, so a
  writer is one more function and not a new driver.
- **The hitbox is a component with a kind.** Step 1's kind is `interaction`;
  step 2's is the mob's own body, and `strike-assembly` then rides the mob's
  hurt like `strike-npc` rides a character's. The object determines the kind
  (the vacuity rule on opt-outs), so the field is `hitbox`, never
  `interaction`.

## 8. Measured on the pinned server

**Cited — `tools/spike-display-assembly/` on `itzg/minecraft-server@sha256:3e7db256…`
(`versions.toml [images.base]`), vanilla 1.21.11, flat world, adventure,
`view-distance=12`, `simulation-distance=10`,
`entity-broadcast-range-percentage=100`; bot mineflayer 4.37.1 (harness pin);
raw readings in `observations.json`, coordinates in `site.json` written by
`gen.py`.** Each row names what was asked and what came back.

| # | asked | came back |
|---|---|---|
| 1 | **Build.** One root `item_display`, 34 `block_display` parts, one `interaction` 3 × 8 at the mark; the bot's entity list after 2 s. | 34 / 1 / 1 on the server; the bot saw 34 block displays, 1 item display, 1 interaction; the frame counter advanced within 1.5 s (`frame_advances: true`). |
| 2 | **Passengers per vehicle.** Three displays mounted one by one on a fresh `item_display`, two on a NoAI zombie; the vehicle's passengers counted by `execute as <vehicle> on passengers run scoreboard players add` (a forked `store result … if entity` stores one branch's `1`, which is how this spec's first run misread it as one). | `item_display`: 3 of 3; zombie: 2 of 2; the assembly's root: **34 of 34**. Every `ride` answered *started riding*. |
| 3 | **Moving the root.** `execute as <root> at @s run tp @s ~2 ~ ~`, then `… tp @s ~-2 ~ ~ 90 0`, then `rotate <root> 45 0`, then `… as <part 17> at @s run tp @s ~ ~ ~ ~ ~`, then remount and `tp @s ~ ~ ~ 0 0`; parts 0, 1, 17, 33 read each time. | +2 x: every sampled part at +2, still mounted; root yaw 0 → 90 by `tp`: every part reads `[90, 0]`; `rotate` 45: the root reads 45, every part still 90; the in-place `tp` of part 17: it reads *no vehicle* and the root counts 33; after remount and the root's `tp … 0 0` (45 → 0): parts read **45** — turned by the root's own change of −45, not set to its yaw — and the root counts 34. A vehicle `tp` turns its passengers by the **difference**; `rotate` turns the vehicle alone. The bot received one `sync_entity_position` for the root and no packet addressed to a part; its stored part positions did not move (a client positions riders from the vehicle; this rig cannot see what it draws). |
| 4 | **Melee.** The bot stands 1.5 blocks from the box's south face in adventure mode and attacks the `interaction` entity 7 times, 600 ms apart, with the poll counting off `attack` and clearing it; one raw record read with the poll paused. | 7 attacks, 7 counted; the raw record is `{player: [I; …], timestamp: 484L}`; the fifth hit switched the clip to `retract` and the sixth and seventh did not switch it back; a hitbox **riding a part** registered one attack as one hit. |
| 5 | **Arrows.** A bow shot by the bot at the box's middle from 6 blocks; a summoned arrow owned by the bot and one owned by nobody, `Motion [0,0,-1.6]`, `NoGravity`, launched 4.5 blocks south of the box's face; the `attack` record read after each; the arrow's rest position classed against the box's z-span. | All three **passed through** (the shot arrow came to rest 77 blocks north of the box, the summoned pair 41 blocks north, none `inGround`); the `attack` record stayed absent for all three; the bot received no damage event. An arrow does not register on an `interaction`. |
| 6 | **The strike pattern** (windup 8 frames, hold 20, strike 10 frames, cadence 5; `natural_health_regeneration` off). The bot stands in the landing box, then in the trigger box outside it, then outside both. | Windup begin → hold begin **36 ticks** (8 frames: the first applies on the next tick, 7 advances × 5); hold → swing **19**; swing → landing **46** (10 frames). In the landing box: health 20 → **14**, the declared 6. In the trigger box outside it: 20 → 20. Outside the trigger box: the landing count stayed at 3 over 5 s and the state machine read idle with the idle clip playing. |
| 7 | **Tick cost** (`tick query`, 100-tick samples after a 6 s window, this workstation, keyframes as one `data merge` per part per frame). | 1 assembly @5 ticks: **6.9 ms** avg (P95 10.8); 1 @1: 8.6 (12.8); 4 @1: 15.3 (20.2); 4 @5: 6.2 (13.4); 10 @5: **9.9** (17.4); 10 @1: 15.7 (24.2); 10 with the animation stopped: 6.8 (9.4). Against the stopped control the ten assemblies cost about **0.3 ms per assembly per tick at a 5-tick cadence and 0.9 ms at every tick**; the server's own idle on this machine is the 6.8. |
| 8 | **Tracking distance.** One assembly left standing; the bot teleported east along a forceloaded lane and its entity list read after 3 s at each stop; `view-distance=12` (192 blocks) so a cutoff under that is the tracker's. | Seen whole (34 parts, hitbox, root) at 40, 56, 64, 72, 80, 96, 112, 128, 144 and **160** blocks; nothing at **176** and 192. The server stops tracking display and interaction entities for a client between 160 and 176 blocks (10 chunks); the client's own `view_range` cull (§9) is inside that. |
| 9 | **Stop.** `kill @e[tag=dwa]`, the lane unloaded. | 0 tagged entities on the server; the bot's list held 0 block displays. |

## 9. Research record

- **Anticipation.** *"Anticipation is used to prepare the audience for an
  action, and to make the action appear more realistic. A dancer jumping off
  the floor has to bend the knees first; a golfer making a swing has to swing
  the club back first."* [cited — Thomas & Johnston, *The Illusion of Life:
  Disney Animation* (1981), as summarised on Wikipedia, *Twelve basic
  principles of animation*]. The windup clip is the anticipation; a strike
  with no windup is refused by §5.4 (3).
- **Reaction time.** *"The average human reaction time, if asked to simply
  press a button on reaction to exactly one easily-identifiable stimulus, is
  around 265 milliseconds"*, *"265 milliseconds converts to approximately 16
  frames"* at 60 fps, *"it is possible to react to moves that have 16 frames
  of startup or more quite consistently"*, and *"Even if the frame data for a
  move indicates it has 30 frames of startup, if the first 20 frames has very
  little movement, then it will be impossible to react"* [cited — Infil, *The
  Complete Killer Instinct Guide*, *On Reaction Times*]. The 10-tick floor is
  authored from these.
- **Triggers and killing volumes.** `docs/reference/reach-and-hazard-volumes.md`
  §1–§2 [cited]: a trigger is a volume the body enters; a killing volume sits
  under the playable area. §5.4 (1) is the first applied to the arming
  region; §5.4 (2) is the second re-stated for a blow that is not a volume.
- **Vanilla facts not measured here, cited to their page**: a display entity
  has no hitbox and no collision (*Display*); `view_range` — *when the
  distance is more than view_range × entityDistanceScaling × 64, the entity is
  not rendered*, default 1.0; `billboard` defaults to `fixed`;
  `interpolation_duration` is in ticks and `start_interpolation` a delay
  before it begins, 0 meaning now (*Display*, entity data, as indexed);
  `teleport_duration` is clamped to `0..=59` and the tween is client-side
  (the engine's own `compiler::camera` measurements 1–3); a player's
  `entity_interaction_range` defaults to 3 (*Attribute*). Interpolation is
  drawn by the client, so **how a clip looks is confirmed only on the demo
  level** — the server and the bot see keyframes, never in-betweens.

## 10. What the gallery, the record, the skill and the demo owe

**Authored.**

- **The gallery element** (spec-0039). One rig under the gallery's own
  prefab generator (`prefabs/gallery-generator`, a four-part rig with `idle`,
  `windup`, `strike` and `retract`, so every clip role is written), one
  assembly in the hall with a hitbox and a one-step strike pattern, a
  `strike-assembly` trigger counting into a `state`, a `play-clip` behind a
  `requires_state` gate, `spawn-assembly` and `despawn-assembly` on two
  beats. Bound by perturbation: changing a frame's translation moves a
  `data merge` line; changing `hold` moves the driver. Units: the object
  class and each of its properties, the three verbs, the trigger kind.
- **The probes** (primary plus one edit, refused by the named code): a
  hitbox of width 7 (`DW_ASSEMBLY_HITBOX`); a landing box outside
  `while_in` (`DW_ASSEMBLY_STRIKE`, shape 1); a landing box under a corner the
  strike clip never reaches (shape 2); a windup of 3 ticks and a hold of 0
  (shape 3); a `play-clip` naming `fly` (`DW_ASSEMBLY_RIG`); a required
  `strike-assembly` whose hitbox stands over the hall's pit beyond reach
  (`DW_ASSEMBLY_REACH`).
- **The record.** `docs/reference/compiler.md`: the class's surface rows, the
  three verbs' and the trigger kind's emission rows, the four codes, the
  binding line, and the measured rows of §8 that emission depends on (a
  vehicle `tp` carries its passengers; a passenger `tp` unseats it; arrows do
  not register). `docs/reference/tools.md`:
  `delvec rig describe` and the spike paragraph (landed with this spec).
- **The skill.** `references/quest-capabilities.md` gains an assembly
  paragraph under *Bodies*: what it is, that it is struck in melee only, that
  a hit count is a `state`, that the strike region is declared from `delvec
  rig describe`'s printed footprint, and the three refusals in the creator's
  words. The harness gains `strike-assembly` in `TRIGGER_KINDS`
  (`harness/src/critical-path.ts`) — an allowlist change the planner hands
  to the implementing round, not consumed here.
- **The bot.** A `trigger` step of kind `strike-assembly` is performed as a
  real attack on the hitbox and passes on the trigger's own marker; a count
  on the critical path is N such steps. A strike pattern on the path is
  judged as spec-0023 judges a fight: the machine proves the loop, not the
  win — the bot must reach and strike, and the staging record states the
  per-strike damage and the caught cells of every landing box.
- **Generated PackTests**, in the family `emit_reseat_undefeated_packtests`
  belongs to: spawn → part count and the root's passenger count; a simulated `attack` record
  written onto the hitbox → the datum moves by one and, at the declared count,
  the clip score reads `retract`; a dummy stood in a landing box → its health
  drops by `amount` on the landing tick and a dummy in `while_in` but outside
  the box does not.
- **The demo level.** Queued in `docs/demo-levels.md` as *The Thing in the
  Pit* (below).

## 11. Decisions for the owner

- **A new object class, `assemblies[]`, with the rig as a library file** —
  the alternative is a shape of `actors[]`, declined in §2 because eleven of
  an actor's fields would mean nothing on it.
- **Struck in melee only.** Measured (§8): an arrow does not write the
  `interaction`'s `attack` record. The alternative is a living invisible body
  as the hitbox, which is a fight class (health, knockback, death, a kill
  credit) and a second hitbox kind on one object. Recommendation: ship melee
  only and say so on the creator's page; a ranged class stands back while a
  melee class does the hitting, which is a party division spec-0018 already
  values.
- **A strike that can kill an unhurt player in one landing** (`amount` ≥ 20)
  — refused, advised or allowed? Recommendation: **advised** (a warning
  naming the step), because the blow is telegraphed and lands only where the
  limb is seen, and the souls line wants hard hits; a refusal would make the
  heaviest attack unauthorable, an allowance would let a one-shot ship
  silently.
- **The telegraph floor of 10 ticks** — authored from a 265 ms average; the
  alternative is no floor, which lets a strike land before anyone could have
  moved.
- **`dsl_version` moves**; no ADR.

## 12. Acceptance criteria

Machine-checkable; each names its instrument. `delvec` is the implementing
tree's own build; the gallery builds from `prefabs/gallery-generator`.

1. **The surface.** `delvec schema --stage all` exports `assemblies[]` with
   `id`, `rig`, `at` (a `Mark`), `facing`, `initial`, `hitbox {width, height,
   offset}` and `strikes {while_in, pattern[] {windup, hold, strike,
   on_land[]}}`; `QuestEffect` gains `spawn-assembly`, `despawn-assembly` and
   `play-clip` (41 verbs); `TriggerOn` gains `strike-assembly {assembly}` (5
   kinds), at the `dsl_version` the crate manifest states. *Instrument: a
   `crates/dsl/tests/` test over the export.*
2. **The rig.** A rig with a part count, clip set and frame shape is parsed;
   a missing clip, an unknown block, a frame short one part, a zero scale and
   a cadence of 0 or 21 are each `DW_ASSEMBLY_RIG` naming the field.
   `delvec rig describe` prints parts, clips with tick lengths, and the
   last-frame footprint per clip; two runs are byte-identical. *Instrument:
   `crates/delvec/tests/`.*
3. **Emission.** A built campaign with one assembly emits one root, one
   `block_display` per part each riding the root (`ride … mount` lines),
   one `interaction` when a hitbox is declared, one
   function per clip frame carrying one `data merge` per part with the
   transform rotated by `facing`, and one driver; `facing: north` moves
   every emitted translation's `x`/`z` against `south`; two builds are
   byte-identical (ADR-0006); every line is walked against
   `CommandTree::v1_21_11`.
4. **Hits.** A `strike-assembly` trigger emits the `attack` poll and clear
   against `dw_asm_<id>_hit`; `once: false` with `add-state` and a gated
   `play-clip` emits the three lines in declared order in one function;
   `audience: presser` on it is `DW0427`; a trigger naming an assembly with no
   hitbox is `DW_ASSEMBLY_HITBOX`; `at` on it is `DW0194`.
5. **The hitbox.** Width 7 and height 23 are `DW_ASSEMBLY_HITBOX`; width 6 and
   height 22 are green; a hitbox disjoint from the initial frame footprint is
   refused naming the footprint's cells; an assembly's hitbox appears in
   `compiler::eclipse`'s enumeration (a body posted on its cell is `DW0359`).
6. **Reach.** A critical-path `strike-assembly` whose hitbox lies more than
   `STRIKE_REACH` from every populated standable cell is `DW_ASSEMBLY_REACH`;
   moving the mark one cell toward the floor greens it; the test reads the
   same constant `strand` reads.
7. **The strike, three shapes.** A landing box one cell outside `while_in`'s
   keep-out is refused (shape 1) and the same box one cell inside is green; a
   landing box under a column the strike clip's last frame never reaches is
   refused naming the cells (shape 2) and the same box under the footprint is
   green; windup 5 ticks + hold 4 is refused and + hold 5 is green (shape 3).
   Each over `compiler::assembly::judge`, plus one end-to-end build per
   shape.
8. **The footprint arithmetic.** A unit test rotates and scales one part and
   asserts its cell set; the same function serves `rig describe` and the
   strike check (one symbol, asserted by call-graph test in the family
   `call_graph_integrity.rs` belongs to).
9. **Generated PackTests** of §10 run in the required `tier 2` job on the
   gallery: the root's passenger count equals the part count; a written `attack` record moves
   the datum by one and the fifth plays `retract`; a landing damages the
   dummy in the box by `amount` and not the dummy outside it.
10. **The binding line** of §5.6 is printed on every build; the gallery
    reports `A = 1`, the primary without the element reports zeroes.
11. **Gallery and probes.** The element of §10 builds green; perturbing a
    frame translation moves a `data merge` line; the six probes are refused
    with their named codes; `check-gallery-coverage.py` reports 0 units in
    neither state.
12. **Record, skill, demo.** The rows of §10 land in the implementing pull
    request; `check-dw-codes.py`, `check-skill-page.py`,
    `check-demo-levels.py` green; the demo row below is queued with this spec.
    *Met for the demo row and the spike paragraph by this spec's own pull
    request; the rest is due with the code.*
13. **The spike is reproducible.** `EULA=TRUE tools/spike-display-assembly/run.sh`
    rewrites `observations.json` with every row of §8 present and no row
    carrying a rejection text where a reading belongs. *Met — this spec's
    pull request commits the run.*
