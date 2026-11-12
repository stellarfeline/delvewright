# A target block, a rocket, a tempted goat, a steered pig, a boat and a minecart — what the pinned server does

Reader: the engine work that proves a level in which a target is shot, a rocket is fired, an animal is lured, a body is ridden, bodies are counted in a volume and an effect is aimed at a mob (spec-0133, spec-0134, spec-0135). Every statement is **[cited]** — measured on the pinned server, Minecraft Java 1.21.11, by `validation/festival-probe.mjs`, with the trial named — or **[authored]**, a judgement made here from the measurements. Established once and treated as constant (CLAUDE.md, *A check guards only what our toolchain owns*); the read site is stated so the reading can be repeated when the pin moves.

## Instrument

- Server: `ghcr.io/stellarfeline/delvewright-base@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de` (`versions.toml`, `[images.base]`), `TYPE=VANILLA VERSION=1.21.11` (the server reports `1.21.11`, data `4671`, protocol `774`), superflat, `difficulty easy`, `spawn_mobs false`, `random_tick_speed 0`, `advance_time false` (the pinned game's name for the daylight rule; `do_daylight_cycle` is refused), noon, a throwaway container with no published port.
- Arena: a stone pad 121 × 121 at y = 100 in open air and open sky, forceloaded; the bot's home at its centre; a basin of water 41 × 31, one deep, flush with the pad; a rail 60 long on the pad.
- Player: one mineflayer 4.37.1 bot (offline mode, survival), the harness's own copy; it is the "player" wherever a player is the question. Its client-side beliefs are reported beside the server's reading where they differ.
- Reading: `data get entity` over rcon through `tools/lib/rcon.mjs`; counts through `execute store result score … if entity` and `scoreboard players get`, because `execute if entity` alone answers rcon nothing (the third run read every count as zero beside a stored score of three). The advancement criteria are read through a datapack the probe writes into the world — one advancement per criterion whose reward counts a score and revokes itself.
- Ten runs; the seventh measured §11's boarding trials under a delve's conditions (T12), the eighth §12 (T13), and the ninth and tenth §13 (T14, T15), each at `validation/festival-probe.mjs` as of the commit that adds its trial. The sixth, at `validation/festival-probe.mjs` as of the commit that adds trials T10 and T11, measured §11 alone. The first five: the first refused at the daylight rule's old name; the second hung on a datapack write; the third measured every trial with counts read the wrong way; the fourth re-read the counts, re-designed the goat trial and added the client-reported moves; the fifth re-ran the pig's obstacles and the boat's speeds without a teleport the client cannot parse (§5, §10).

## 1. A target block **[cited, T1]**

- **`minecraft:target_hit` is granted to the player who shot.** An arrow from the bot's bow, drawn 1.2 s and released at a target block 27 blocks east (its centre one block over the bot's feet), counted one hit each time it struck; an arrow summoned 6 blocks west of the block with a motion toward it and no shooter counted nothing in three tries; a snowball the bot threw from 6 blocks counted one hit on the throw that struck (one of three — the other two missed).
- **The aim that hits at 27 blocks.** Aimed at the block's centre raised 1.5, 2.0 and 2.5 blocks (pitch 3.0°, 4.0°, 5.1° above level), both shots at each elevation struck; raised 0, 1, 3 and 4 blocks, none did. At 2.0 both shots also met `signal_strength` of at least 8 — the centre; at 1.5 and 2.5 neither did, the rim.
- [authored] The static proof of a shot is a clear line from a standing cell, and the bot's leg is a bow drawn full and aimed about two blocks over the centre at 27 blocks, with the elevations either side as the next tries. A shot nobody fired is not a hit; the engine's own projectiles cannot stand in for the player's.

## 2. A firework rocket **[cited, T2]**

- **The burst's damage rises with the star count.** A rocket summoned inside a NoAI pig of 200 health with `LifeTime` 1, by explosions on its `minecraft:fireworks` component: 0 stars, no damage; 1 star 6.7; 2 stars 8.6; 3 stars 10.6; 4 stars 12.5; 5 stars 14.4; 6 stars 16.3; 7 stars 18.2 — two more a star, from about seven.
- **A crossbow the bot loads fires it.** The bot held a crossbow, eight 3-star rockets in its off hand, charged 1.5 s and released, then used the crossbow once at the pig 6 blocks east: the pig lost 10.5, the bot standing 6 blocks off lost nothing.
- [authored] A rocket with no stars is a flare; the stars are the weapon, and the item the campaign hands over must carry them (spec-0134 §5). A bot can fire a crossbow; the engine does not yet ask it to.

## 3. A goat beside wheat **[cited, T3, fourth run]**

- **A goat is tempted by wheat and follows it.** A goat summoned 9 blocks south of a bot holding wheat came to 2.1 blocks within the first seconds and held there for 12 s; the bot moved 8 blocks west, and the goat came to 2.2 blocks of it again and held — in all three trials. Holding a stone instead, the goat wandered: once past the bot (0.4) and off to 11.2 blocks after the move; once never nearer than 9.0; it followed the moved bot in neither.
- [authored] Luring is the vanilla tempt, reliable at this range; the engine does not reproduce it in the bot and proves the pen by the ruling of spec-0134 §1.

## 4. A saddled pig under a rider **[cited, T4]**

- **Mounting.** A right-click on a saddled pig (`equipment:{saddle:…}`) seats the bot: the server's `RootVehicle` reads `minecraft:pig` and the client agrees; the rider's position on the server is the pig's cell, 0.27 over its feet.
- **The rider's input does not steer it.** Holding a carrot on a stick and looking east, with the forward input sent twenty times a second for 6 s, the pig did not move. With the hand emptied the pig walked 5.8 blocks under the rider in 4 s on its own.
- **The rider's client moves it.** The pig's position reported by the rider's client (`vehicle_move`, 0.2 blocks east a tick for 2 s, 8 blocks) is what the server holds: the pig stood 8 blocks east, the rider on it.
- **Obstacles under a reported move** (fifth run; the pig never teleported, each obstacle built three blocks ahead of where it stood, reads of the server's pig every 0.2 s). A one-block step, reported through at the pig's own height: the server held the pig a reading at the step's face, then took the positions beyond it — 8 blocks on, back on the ground. A two-block wall: the server held the pig at the wall's face and took nothing beyond; 2 blocks of 8. A bottom slab then a full block, reported flat: held at the slab's face, 2 of 8. The same slab and block with the report rising 0.5 at the slab and 1.0 at the block: taken whole, the pig standing on the block at y = 102. The pig's own AI never drove any of these; the rider's client did.
- **Leaving.** mineflayer's `dismount` sends the jump bit of this version's `player_input` packet and leaves the rider seated; the sneak bit dismounts on the server (`RootVehicle` none) while the client's `bot.vehicle` stays stale; the body stands one block beside the pig. `ride <player> mount <pig>` seats and `ride <player> dismount` unseats, and the client sees both.
- [authored] A ridden mob is moved by its rider's client; the harness drives a mount by reporting its position along the proven route and reads every verdict on the server (spec-0135 §6).

## 5. A boat **[cited, T5]**

- **It floats where the water puts it.** Summoned at y = 101 over water flush with the pad, it settled at y = 100.52.
- **Mounting and the rider's place.** A right-click seats the bot; `RootVehicle` reads `minecraft:oak_boat`; the rider's position on the server is the boat's cell at y = 100.11 (the client agrees); `execute as <player> on vehicle if entity @s[type=minecraft:oak_boat]` stores 1.
- **The rider's input does not move it.** The forward input for 4 s moved the boat nothing.
- **A `Motion` written to the boat moves it with its rider**: 0.6 south a tick carried boat and rider 28.5 blocks to the basin's end.
- **The rider's client moves it, at every speed tried.** Fifth run, in sequence from where the boat was, the client's and the server's boat agreeing at each start: reported positions south at 2 blocks a second for 1 s (2 blocks), 5 for 1 s (5), 10 for 1 s (10) and 20 for half a second (10) were each taken whole — the server's boat and rider exactly where the last report put them. (The fourth run's 5 and 10 readings are void: the boat had been teleported between them and the client, which cannot parse the teleport, reported from the cell it last knew.)
- **A teleport carries the rider**, by one `tp` and by a chain of ten a tenth of a second apart: the rider's server position followed the boat each time and the seat held. The client did not follow: mineflayer 4.37.1 fails to parse this version's `entity_teleport` (`Chunk size is 63 but only 29 was read`) and keeps the old position — every vehicle position the harness acts on is read from the server.
- **Leaving in open water** by the sneak bit (the jump bit does nothing here either) stands the body **on the boat**, at y = 101.10 over a boat at 100.52, not in the water. `ride … dismount` does the same.
- **A rider is counted in a box**: `@a[name=…,x,y,z,dx,dy,dz]` over the basin counted the seated bot (fifth run; the fourth run's zero was a selector the server refused, a bare name with arguments).
- [authored] A boat is driven by its rider's client; the server takes a reported move at the speeds the harness would ever report; a dismount in open water puts the body on the hull, from which the next step is a swim (spec-0134 §10) or a re-mount.

## 6. A minecart **[cited, T6]**

- **Mounting** by a right-click; `RootVehicle` reads `minecraft:minecart`; `on vehicle` stores 1; the rider sits at the cart's cell, 0.65 under its own feet reading.
- **The rider's input barely moves it**: the forward input for 3 s moved it 0.23 blocks on plain rail. **The rider's client does not move it**: reported moves of 8 blocks along the rail left the server's cart 0.37 blocks on — a minecart is the server's.
- **The rails move it.** Given a motion of 0.3 east a tick on plain rail with the rider aboard, it ran three powered rails on redstone blocks at 6–8 blocks and reached 10.4 blocks a second (read every half second), held that speed over the 45 blocks of plain rail that followed, and stopped only two blocks past the rail's end.
- **A teleport carries the rider**, as for the boat. **Leaving** by the sneak bit stands the body beside the rail, one cell off; `ride … dismount` does the same.
- [authored] A minecart's driver is its rails: a ride leg needs a push and powered rails where the run must not die, and on flat plain rail a ridden cart at speed loses almost nothing over forty-five blocks (spec-0135 §3).

## 7. A box selector over bodies **[cited, T7, fourth run]**

- **A body is counted by its hitbox, over the box `[x, x + dx + 1)`.** With `x=230,dx=6` (and the like on z and y), three NoAI goats inside counted 3; a fourth whose centre stood at 237.3 (hitbox 236.85–237.75) counted, at 237.6 (237.15–238.05) did not; at 229.7 (229.25–230.15) counted, at 229.4 (228.95–229.85) did not. The box's faces are at 230.0 and 237.0.
- **A teleport in or out is seen on the next read**: one goat teleported out, 2; back in, 3. `execute store result score … if entity` wrote 3.
- [authored] A census region is the selector box, and a body on its rim counts when its hitbox reaches in (spec-0134 §1); the harness's census teleport is read by the game as any arrival is.

## 8. An effect and a blow aimed at a mob **[cited, T8]**

- `effect give @e[tag=…] minecraft:glowing 10 0 true` applied (`active_effects` reads the id and 192 ticks left a few ticks on).
- `damage @e[tag=…] 3 minecraft:generic` took 3 health; `damage … 3 minecraft:player_attack by <player>` took 3 more.
- **Knockback needs a body that moves**: the NoAI pig did not move under either; a pig with its AI (speed 0) did not move under the plain blow and was knocked 1.99 blocks, away from the player, under the blow `by` the player.
- [authored] A body audience on `give-effect` and `damage` is the game's own targeting (spec-0134 §9); knockback is the `by` form on a body with AI, which a puppet is not.

## 9. A spyglass **[cited, T9, third run]**

- **`minecraft:using_item` is granted every tick a spyglass is held in use**: 1.5 s of use counted 30, the advancement re-armed by its own reward each tick, as the compass's is.
- [authored] A `use-item` trigger is one advancement with the item and the place (spec-0134 §3).

## 10. What the engine takes from it **[authored]**

- A shot is the player's or nobody's; the bot lands one about two blocks over the centre at 27 blocks (§1). A rocket's stars are its damage (§2). Wheat lures a goat and keeps it (§3).
- A ridden body — boat or mount — is where its rider's client says it is, up to a step the server's own collision refuses; a minecart is where its rails put it; a teleport carries a rider and a dismount is the sneak bit, read on the server (§4–§6). The harness therefore drives a boat and a mount by reporting positions along the proven route with the route's own heights, lets the rails drive a cart, and reads every position from the server, because its client neither simulates a vehicle nor parses this version's entity teleport.
- A census box is `[x, x + dx + 1)` by hitbox (§7); an effect and a blow aimed by tag are the game's own (§8); a raised spyglass is seen every tick (§9).
- [authored] spec-0135 rests two facts on this record and never re-checks them: that `ride … mount` seats a body, `RootVehicle` reads the carrier and `ride … dismount` stands it beside the vehicle (§4–§6), and that the server takes a client-reported move of a boat or a mount whole (§4, §5). The bot's ride step reads every verdict on the server; it does not re-measure either.

## 11. A cart at rest, a cart pushed once, and a mount that waits **[cited, T10, T11, sixth run]**

- **A powered launcher runs an empty cart away.** A powered rail on a redstone block (`powered=true`) with a stone block behind it and plain rail ahead: a minecart summoned on it with nobody in it left at once, at 4.6 blocks a second in the first 0.7 s, and came to rest 7.98 blocks on. A cart seated on a launcher is not at its seat when a body comes for it.
- **The same launcher under a rider** carried cart and rider 47 blocks in 14 s and was still rolling at 2.1 blocks a second when the trial ended; its speed fell from 4.8 blocks a second in the second half-second to 2.2 at 43 blocks.
- **A cart on level plain rail stays.** Nobody in it for 4 s: moved 0.00. The bot boarded by a click (`RootVehicle` the minecart, the `minecraft:started_riding` criterion counted once) and sat 4 s: moved 0.00.
- **One push carries a ridden cart the whole of a plain line.** The seated cart given `Motion:[0.3d,0.0d,0.0d]` once, on 57 cells of plain rail with no powered rail: 4.4 blocks a second in the first second, 2.2 at 38 blocks, 1.1 at 57, and it stopped at the rail's end, 57.22 blocks on, 23.5 s after the push — a mean of 2.4 blocks a second over the whole line, the slowest whole run this record holds.
- **The push can be the boarding.** A cart at rest tagged for the reward of a `minecraft:started_riding` advancement, whose function writes the same motion to the rider's vehicle (`execute on vehicle run data merge entity @s[tag=…] {Motion:[0.3d,0.0d,0.0d]}`) and revokes the advancement: the bot boarded by a click and the cart ran — 4.25 blocks in the first second, 37.6 in 13 s. After the rider left by `ride … dismount` the cart rolled 1.9 blocks more and stopped.
- **A mount whose speed is nothing waits.** Two saddled pigs with their AI, 40 s: the one as summoned wandered 8.71 blocks from where it stood; the one with `attribute … minecraft:movement_speed modifier add <id> -1 add_multiplied_total` (its speed read 0 against 0.25) moved 0.00.
- **Boarding gives the speed back, and the rider's client moves the mount at either speed.** The `started_riding` reward removing the modifier from the rider's vehicle: the pig's speed read 0 before the click and 0.25 after it. Forty reported moves of 0.2 blocks (8 blocks) were taken whole by the server after boarding (speed 0.25) and, in a second trial with the modifier left on, at speed 0 as well: a client-reported move does not read the mount's speed.
- **`minecraft:started_riding` fires for a boat too**: counted once when the bot boarded an oak boat by a click.
- [authored] A cart is seated on level rail that does not launch it, and is pushed when a body boards; a riderless mount is held at its seat without its AI, which boarding gives back (§13). A ride on a cart is priced and waited for at 2.4 blocks a second, the slowest whole run above, and a stretch of rail that does not drive the cart is taken to carry it 45 cells (§6 held speed over 45; the one push above was still rolling at 57).

- **Under a delve's conditions (T12, seventh run).** The summon a build emits (`{Invulnerable:1b,Rotation:[0f,0f],Tags:[…]}` at the cell's floor) on a north-south line of twelve with a stop past its end: boarded by a click from the cell beside it, standing or walking up, the cart read a motion of 0.29 a tenth of a second after the click and ran the twelve cells to the stop in under three seconds.
- **A body that walks into a resting cart shoves it.** The bot walking into the cart for a second and a half moved it 4.7 cells along its rail before any click (T12). On a built gallery's server, read by the bot's own ride step, a cart resting on the first cell of its line with nothing behind it stood 0.75 of a block **off the end of its rail**, on the floor (`OnGround` 1), a quarter of a second after the click that boarded it — the arriving bot had rolled it back off the line — and the boarding push, written to a cart on the floor, was gone within the same reading (motion 0.015); the cart slid back onto its rail and crept. With the same push written four seconds later, on the rail, it ran the line.
- [authored] A cart's mark has the rail going on, or a solid block standing, behind it (else `DW1097`), and a walk to any vehicle goes round the cells its body stands in — in the proof, and in the bot's walk by the cells the `ride` step names.

## 12. A camel's seats by a click **[cited, T13, eighth run]**

- A tamed, saddled camel, three players standing beside it, each clicking it in turn; the riders counted from each player's side (`execute as @a on vehicle if entity @s[tag=…]`), because a player's saved `RootVehicle` is written only while exactly one player is aboard. After the first click one player rides; after the second, two; **after the third, still two**. `ride <third> mount <camel>` then seats the third: three ride.
- [authored] The seats a party takes are the seats a click gives. `Entity#canAddPassenger` admits three on a camel and the click admits two, so the steering table carries two for a camel (`tools/maintenance/extract-entity-steering.py`, `SEATS_BY_CLICK`).

## 13. The yaw a waiting mount holds **[cited, T14, T15, ninth and tenth runs]**

Read site: `validation/festival-probe.mjs` T14 and T15; the ninth run (T14) at the commit that adds T14, the tenth (T14 and T15) at the commit that adds T15. Each pig is the summon a build seated a mount with before this record (`Invulnerable:1b,PersistenceRequired:1b,Silent:1b,Rotation:[<yaw>f,0f]`, a saddle, the `movement_speed` modifier at −1 `add_multiplied_total`); `Rotation` was read by `data get entity` every second, and after `save-all flush` from the save's entity region file through `tools/lib/anvil.py` — a reading that shares nothing with `data get`.

- **A mount with its AI turns, with nobody near and at a speed of nothing.** With the bot 400 blocks off, four waiting pigs summoned at yaw 0, 90, 180 and 270: in the ninth run they read 0, 90, 45, 270 one second after the summon and 45, 135, 45, 270 at thirty seconds; in the tenth, 225, 90, 180, 315 at one second and 225, 315, 90, 315 at thirty. None moved from its cell (0.00). The turns are whole multiples of 45 degrees and differ between the two runs. A pig with no modifier turned and wandered (13.56 and 5.86 blocks).
- **The save holds the turn.** The region file read back the live yaw of every pig in both runs (ninth: 45, 135, 45, 270; tenth: 225, 315, 90, 315, the last a 89.99998 the file states for 90), not the yaw the summon wrote. The pitch read 0 in the save; a bot beside the row moved only the pitch while it stood there (−13.34 at most).
- **A mount without its AI holds its yaw.** The same summon with `NoAI:1b` read yaw 0 every second in both runs and in the save. With a waiting pig and a tame horse summoned at yaw 90 with `NoAI:1b` (T15), each read `[90.0f, 0.0f]` and stood at its cell for five seconds.
- **It is boarded by a click, and its AI comes back by a command.** The bot clicked each, holding a carrot on a stick for the pig and nothing for the horse: the server's `RootVehicle` named it, `minecraft:started_riding` counted once, and the advancement's reward `execute on vehicle run data merge entity @s[tag=…] {NoAI:0b}` left no `NoAI` key on it; forty client-reported moves of 0.2 blocks were then taken whole by the server for each (8.00 blocks).
- [authored] A mount the engine seats waits without its AI, so the facing it is declared with is the facing the game shows and the save holds; boarding gives the AI back. The speed modifier is no longer the wait.
