# What a skinned NPC body can be made to do — the pinned game's mannequin, measured

The vanilla record behind a staged body's moves (spec-0136: `hop`, `pose`, `turn`, `face`, `hold`, `resize`, a particle and a sound at a body). Reader: the agent changing what the engine emits for a body that moves where it stands. Every fact is **[measured]** (command and observation named, run on the pinned server) or **[cited]** (class and member named, read from the pinned jar), or **[authored]** (a judgement). Nothing here is a recurring check.

## Instrument

- Server: `itzg/minecraft-server:java21`, `TYPE=VANILLA VERSION=1.21.11`, superflat (floor top y = -60), `PAUSE_WHEN_EMPTY_SECONDS=-1`, `spawn_mobs false`, one throwaway container with a non-default host port. Jar sha256 `f83b8e09…1726` (bundler), inner `server-1.21.11.jar` `ec47239a…fcebc`, both equal to the pins in `body-movement.md` §0. Mappings sha1 `5621e925…5955` (same as the pin). The decompiled jar was read for citation only.
- Body: summoned as `crates/delvec/src/compiler/emit/npc.rs` `npc_summon_rows` summons a skinned NPC: `summon minecraft:mannequin X Y Z {profile:{texture:"delvewright:npc/<id>",model:"<wide|slim>"},immovable:1b,pose:"standing",Invulnerable:1b,Silent:1b,Rotation:[<yaw>f,0f],description:{text:"<name>"},Tags:["dw_npc","<tag>"]}`. Trials used `profile:{name:"Steve",model:"wide"}` (the texture form was also summoned and accepted, and reads back as written). The engine's body has **no** `NoGravity` and no `NoAI`; `immovable:1b` is its only freeze.
- State: `data get entity`, and a per-tick datapack `tick` function that appended `Pos`/`pose`/attributes to a storage list, read once at the end (so each row is one server tick, no rcon timing). A server with no player pauses after 60 s (`pause-when-empty-seconds`, default 60): a mannequin read in such a server does not tick at all. [measured: a body read in a paused server does not move until the setting is -1]
- Client: one mineflayer 4.37.1 bot (offline) joined and logged every clientbound packet naming the mannequin's entity id (`entity_metadata`, `entity_look`, `entity_head_rotation`, `rel_entity_move`, `entity_equipment`, `entity_update_attributes`, `damage_event`, `animation`), plus `world_particles` and `sound_effect`. That is what the client is told, **not a picture**: no frame was looked at, and no visual below is claimed. mineflayer's attribute-name table is mis-keyed for 1.21.11 (a scale value arrives as `submerged_mining_speed`); only the values are used.
- §9 was measured on a second container of the same image and version, with no client: a per-tick function appended each body's `Pos` and `immovable` to a storage list, read once at the end; the chunk was held by `forceload`.

## 0. The body as the engine summons it

- Saved fields of a mannequin: `profile`, `hidden_layers`, `main_hand`, `pose`, `immovable`, `description`, `hide_description`, plus the living-entity set (`Pos`, `Motion`, `Rotation`, `Health`, `attributes`, `equipment`, `Glowing`, `Tags`, …). [cited `net.minecraft.world.entity.decoration.Mannequin` `addAdditionalSaveData` / `readAdditionalSaveData` (obf `cze`)] [measured: `data get entity` of a live body lists exactly these]
- `immovable:1b` makes `isImmobile()` true and `isEffectiveAi()` false. [cited `Mannequin.isImmobile`, `Mannequin.isEffectiveAi`]
- Spawn defaults: `attributes` holds `movement_speed 0.7`; `scale`, `gravity`, `jump_strength` (0.42) are present on read. [measured]
- `EntityType.MANNEQUIN`: 0.6 × 1.8, `clientTrackingRange(32)`, `updateInterval(2)`. [cited `EntityType` registration of `mannequin`] Position, rotation and attribute changes therefore reach a client at most every second tick (§1, §3, §5); `entity_metadata` (pose, equipment, flags, description) reaches it every tick. [measured]

## 1. A hop

- **With the engine's NBT as it is, `Motion` does nothing.** A body at the floor given `Motion:[0,0.42,0]` stayed at y = -60 for 24 ticks, `Motion` unchanged, `OnGround` 0b. A control body at y = -50 (also `immovable:1b`) hung in the air for the whole run; a control with `immovable:0b` fell to the floor. The single property is `immovable`. [measured: `data merge entity <t> {Motion:[0.0d,0.42d,0.0d]}` ; `summon` of three controls]
- **Release, hop, re-lock works.** `data merge entity <t> {immovable:0b,Motion:[0.0d,0.42d,0.0d]}`; the body rose and fell under gravity, landed on the same cell (`x` 0.5 on every tick), and `data merge entity <t> {immovable:1b}` held it where it stood. Per tick, floor-relative y: 0, 0.42, 0.7532, 1.0013, 1.1661, 1.2492, 1.2522, 1.1768, 1.0244, 0.7967, 0.4952, 0.1213, 0 — peak 1.2522 at tick 6, on the ground again at tick 12 (0.6 s). [measured, per-tick storage log] This is the vanilla jump, as `body-movement.md` records for a player.
- **A taller or floatier hop: the `gravity` attribute.** `attribute <t> minecraft:gravity base set 0.04` before the same impulse: peak 2.17 above the floor, airborne 20 ticks. [measured] Reset with `base set 0.08`.
- **A hop that never releases the body: `tp` per tick.** With `immovable:1b` throughout, a function that issued `tp <t> 0.5 <y> 0.5` once per tick through the 12 y values above reproduced the arc exactly (server log identical), ended at the floor, and `immovable` read 1b after. [measured] The body stays wherever the last `tp` put it (a final value below the floor leaves it in the floor: the arc must end at the floor y).
- **What a client sees.** Position changes arrive as `rel_entity_move` every second tick (about 100 ms) with the accumulated delta, e.g. 0.753 after the first two ticks; the client interpolates between them. [measured; cited `updateInterval(2)`] A 12-tick hop is about six samples. Not viewed.
- **Repeat rate.** Back-to-back hops need the landing tick (12) to pass. [measured arc length]
- Other primitive tried, no result: the `levitation` effect was accepted but was applied to a body that was `immovable:1b`, so nothing was learned from it. Unmeasured.

## 2. Poses

- **Accepted values: `standing`, `crouching`, `swimming`, `fall_flying`, `sleeping`.** [cited `Mannequin.VALID_POSES` = {STANDING, CROUCHING, SWIMMING, FALL_FLYING, SLEEPING}, `POSE_CODEC` validates against it] [measured: each set with `data merge entity <t> {pose:"…"}`, read back, and still read the same 3 s later]
- **Every other pose name is accepted by the command and silently becomes `standing`.** `spin_attack`, `long_jumping`, `dying`, `croaking`, `using_tongue`, `sitting`, `roaring`, `sniffing`, `emerging`, `digging`, `sliding`, `shooting`, `inhaling` each read back `"standing"`, whatever the pose before. [measured; cited `readAdditionalSaveData` `pose` `.orElse(STANDING)`] A typo resets the body; the engine must name the five.
- **Alternation speed: every tick.** A function writing `crouching` on even ticks and `standing` on odd ticks read `standing, crouching, standing, …` on 18 consecutive ticks, and the client received one `entity_metadata` pose packet (5 = crouching, 0 = standing) per tick, about 50 ms apart. [measured] The bounce is a crouch–stand; whether the client animates a 1-tick flip legibly is not viewed.
- The `minecraft:interaction` hitbox the engine puts at the body is a separate entity and does not follow the pose. [cited `npc_summon_rows`]

## 3. Facing

- `rotate <t> facing X Y Z`, `rotate <t> <yaw> <pitch>`, `tp <t> ~ ~ ~ facing X Y Z`, `tp <t> X Y Z <yaw> <pitch>` and `data merge entity <t> {Rotation:[…]}` all turn an `immovable:1b` body; `Rotation` reads back (e.g. facing +x reads `-90.0`, facing +z reads `~0`, facing a point above reads pitch -45). [measured]
- Each is followed by one `entity_look` and one `entity_head_rotation` carrying the same yaw (and pitch): the head and body are turned together; there is no command here that turns the head alone. [measured packets]
- A spin of 20° per tick (`tp @s ~ ~ ~ ~20 0` each tick) read 20, 40, … 360 per tick on the server; the client received look/head packets every second tick (about 40° apart). [measured]
- `tp … facing entity @p` fails ("No entity was found") when no player is online: a face-the-player rule needs a player. [measured]

## 4. Held and worn items

- `item replace entity <t> weapon.mainhand|weapon.offhand|armor.head|armor.chest|saddle with <item>` is accepted (banner, pumpkin, poppy, torch, firework rocket, leather chestplate tried); `body` is not a slot ("Unknown slot"). Read back under `equipment` (`mainhand`, `offhand`, `head`, `chest`, `saddle`); each change reaches the client as an `entity_equipment` packet the same tick. `… with minecraft:air` clears it. [measured]
- Per-tick swap: mainhand `torch` ↔ `air` on alternate ticks reached the client as one `entity_equipment` packet per tick. [measured packets; not viewed]
- A banner or flower on the head is accepted as data; how it renders on a mannequin head is not viewed.

## 5. Size

- `attribute <t> minecraft:scale base set <v>`; read back by `attribute … get` and in `attributes`. 1.5 → 1.5, 0.5 → 0.5, 17 → 16, 0.01 → 0.0625: effective range 0.0625 to 16. [measured]
- A change reaches the client as an attribute update on the entity's next sync tick (every second tick, §0): scale alternated every tick (1.0 / 1.25) was seen by the client only as 1.25, repeatedly; alternated every second tick (two ticks at each) the client saw both values, one change per ~100 ms. So a scale pulse needs each value held at least 2 ticks. [measured packets]
- A large body overlaps and pushes players: at scale 17 the bot received repeated `entity_velocity` shoves from the (immovable) body. [measured] Keep a pulse small (about 1.0–1.25).

## 6. Arm swing, wave, any limb animation

- **Nothing triggers one.** The swing is the `animation` packet sent by `LivingEntity.swing(InteractionHand)`. [cited `LivingEntity` (obf `chl`), the `ClientboundAnimatePacket` constructed in `swing`] None of the 144 classes in the pinned server jar that name `InteractionHand` is a command class or `Mannequin`. [measured: `grep` over the decompiled jar, classes mapped to names] On the live server, none of `damage`, `item replace`, `data merge` of a swing/hurt-time field, `rotate`, `tp` produced an `animation` packet for the body.
- The only body reaction a command does cause is the hurt reaction: `damage <t> 1 minecraft:player_attack by <player>` reaches an `Invulnerable` body (a hit by a creative player ignores invulnerability; `minecraft:generic` and `thorns` are refused) and sends `damage_event` plus a health change, and the body loses health. [measured] That reads as a blow, not joy; it is not a celebrate move.
- Held-item arm pose, crouching and sleeping are the available "limb" changes (§2, §4). [authored]

## 7. Around the body

- `data merge entity <t> {Glowing:1b}` → entity flags byte 64 sent; `{Glowing:0b}` clears. The glow colour follows the scoreboard team colour (not tried). [measured flag]
- `execute at <t> run particle minecraft:happy_villager ~ ~1 ~ 0.3 0.5 0.3 0 12`, `minecraft:note ~ ~2.2 ~ 0.2 0.1 0.2 1 3`, `minecraft:heart ~ ~2 ~ 0.3 0.3 0.3 0 5`, `minecraft:firework ~ ~ ~ 0.3 0.3 0.3 0.1 8` were each accepted; the client received a `world_particles` packet at exactly the body's x, z and the stated height. Not viewed. [measured]
- `execute at <t> run playsound minecraft:entity.villager.celebrate neutral @a ~ ~ ~ 1 1`, `minecraft:block.note_block.bell record @a ~ ~ ~ 1 1.2`, `minecraft:entity.firework_rocket.blast ambient @a ~ ~ ~ 1 1` each accepted; the `sound_effect` packet's position was the body's (0.5, -60, 0.5). Not heard. [measured]

## 8. The rest of the mannequin

- **Label as speech bubble:** `data merge entity <t> {description:{text:"Hooray!",color:"gold"}}` changes the text above the head at once (an `entity_metadata` packet). **`data merge` merges compounds, so a later `description:{text:"…"}` keeps the earlier `color`**; to change the colour back set it explicitly. `hide_description:1b` removes it (packet with no value); `hide_description:0b` afterward restores the **default label** (`entity.minecraft.mannequin.label`), not the custom text — the custom text must be set again. [measured; cited `Mannequin.addAdditionalSaveData` writes `hide_description` and omits `description` while hidden]
- **`main_hand`:** `"left"` / `"right"` accepted and sent. Mirrors which arm holds the main-hand item. [measured]
- **`hidden_layers`:** a list of player model parts (`"hat"`, `"jacket"` tried); the metadata byte changes (127 → 61 → 127). A blink or an outfit switch is possible as data; look not viewed. [measured]
- **`profile`:** switching `name` and `model` (`wide`/`slim`) at runtime is accepted and sent; `texture` from the engine's pack form is accepted on summon. [measured]
- **Pose `fall_flying` and `swimming` with an `armor.chest` elytra:** not tried.
- Living-entity primitives also present (not built on): `effect`, `attribute` (`gravity`, `jump_strength`, `movement_speed`), `Fire`, `Air`. [measured: listed in the dump]

## 9. The hop the engine emits, and the moves beside it

Measured for spec-0136 on the pinned server; the arc is `dsl::moves::hop_arc`'s (the §1 heights rounded to 1/1024 of a block), walked by the function shape `emit::moves::hop_fns` emits: a per-body count in a `dummy` objective, and each tick `execute as @e[scores={<objective>=1..}] at @s run` one `tp @s ~ ~<step> ~` chosen by the count, then the count down.

- **Three bodies, one arc, each back on its exact height.** A mannequin with the engine's NBT at y -60 (a full block), one at y -59.5 on a slab, and a villager with the engine's unskinned NBT (`NoAI:1b,NoGravity:1b`) at y -60 were each started once. Each read, tick by tick above its start: 0.419921875, 0.7529296875, 1.0009765625, 1.166015625, 1.2490234375, 1.251953125, 1.1767578125, 1.0244140625, 0.796875, 0.4951171875, 0.12109375, 0 — and on the twelfth tick `Pos[1]` was `-60.0d`, `-59.5d` and `-60.0d` again, the digits it started with. `x` and `z` read `0.5` (and `2.5`, `4.5`) on every tick. [measured: 50 logged ticks per body]
- **Never released.** The mannequin's `immovable` read `1b` on 50 of 50 logged ticks, through the hop and after it; its `Rotation` read `[37.0f, 0.0f]` before and after: a `tp` with no rotation argument keeps the facing. [measured]
- **A body with no count takes the hop.** `execute as <body> unless score @s <objective> matches 1.. run …` ran for a body that had never held a score: a missing entry fails `matches`, so the `unless` passes. A body already counting is skipped. [measured]
- **`Motion` cannot hop an unskinned body.** `data merge entity <villager> {Motion:[0.0d,0.42d,0.0d]}` on the `NoAI`, `NoGravity` villager: `Pos` unchanged one second later. The teleport is the one primitive that moves every body the engine stages. [measured]
- **Released bodies push each other.** Two mannequins summoned `immovable:0b` at x 8.5 and 8.8 stood at x 8.156 and 9.235 two seconds later (0.3 apart became 1.08); two summoned `immovable:1b` at x 12.5 and 12.8 read 12.5 and 12.8. A crowd released to hop does not stand where it was put. [measured]
- **Turning.** `execute as <body> at @s run rotate @s ~20 ~` took `Rotation` from 37 to 57, and `~-45` to 12: a relative yaw, the pitch kept. `execute as <body> run rotate @s -90 0` read `[-90.0f, 0.0f]`. Facing a point six blocks out and 5.5 up: with `anchored eyes` the pitch read -32.89, without it -42.51 — the anchor picks the body's eyes, as it does for a watched player. [measured]
- **Every equipment slot.** `item replace entity <mannequin> armor.legs|armor.feet|armor.body|saddle with <item>` are accepted beside the four of §4 and read back under `equipment` as `legs`, `feet`, `body`, `saddle`; the slot §4 found unknown is spelled `armor.body`. `… with minecraft:air` empties one. Whether the game draws an item there is the equipment fit rule's question (`DW0898`), not the command's. [measured]
- **Size, pose, particle, sound as emitted.** `execute as <body> run attribute @s minecraft:scale base set 1.25` reads back 1.25; `data merge entity <body> {pose:"fall_flying"}` reads back `"fall_flying"`; `execute at <body> run particle minecraft:heart ~ ~2 ~ 0 0 0 0 5 force @a` and `execute at <body> run playsound minecraft:entity.villager.celebrate master @a ~ ~ ~ 1 1.2` are accepted with no player online. [measured]

## Open questions

- Pose, scale and item changes were not looked at on a client. A look at a 12-tick `tp` arc, a 2-tick crouch flip, a 20°/tick spin, a 1.15 scale pulse and the `sleeping` / `swimming` / `fall_flying` bodies decides which read as joy rather than as glitch.
- Whether the client-side interpolation of a 10 Hz `tp` arc reads as a smooth hop or as a stutter.
- The head banner and flower on a mannequin head, and an elytra with `fall_flying`, as rendered.
- `levitation` / `slow_falling` / `jump_boost` on a released body, and a team colour for the `Glowing` outline.
- A hop under a ceiling lower than its peak: the teleport passes the head through the block (a `tp` collides with nothing). Not measured here; the build refuses it at the body's placed cell (`DW1103`).
