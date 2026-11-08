# The warden as a hunter — what the pinned server does

Reader: the agent designing a level where a warden hunts the players and the players steer it, and the engine work that proves such a level. Every statement is **[cited]** — measured on the pinned server, Minecraft Java 1.21.11, by `validation/warden-maze-probe.mjs`, with the trial named — or **[authored]**, a judgement made here from the measurements. The published numbers (Minecraft Wiki, *Warden*; CC BY-NC-SA 3.0, ideas only) are quoted only where a measurement confirms them. What the compiler emits for a warden — the `dig_cooldown` memory a command summon lacks, the seeded anger of a click trigger — is `compiler.md`, the `unleash-actor` row, and is not restated here. The maze these facts size is `maze-craft.md`.

## Instrument

- Server: `ghcr.io/stellarfeline/delvewright-base@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de` (`versions.toml`, `[images.base]`), `TYPE=VANILLA VERSION=1.21.11`, superflat, `difficulty easy`, `spawn_mobs false`, `random_tick_speed 0`, a throwaway container with no published port.
- Arena: a stone pad 91 × 91 at y = 100 in open air and open sky, forceloaded; the warden stands at its centre and is read over rcon (`data get entity`) every 0.5–1 s. Every warden is summoned with `PersistenceRequired` and the `dig_cooldown` memory, so it neither despawns nor digs during a trial.
- Player: one mineflayer 4.37.1 bot (offline mode), parked 40 blocks from the warden except where the trial puts it closer; it is the "player" wherever a player is the question.
- Reading: the warden's feet. `Brain.memories."minecraft:disturbance_location"` is not saved to NBT on this version and reads back null while the warden walks to the spot, so "heard" is **the warden reaching within 2 blocks of the sound's cell**, and the control trials (no sound) bound what roaming alone does.
- Runs: run 1 (every trial; the hunting trials void, see §3) and run 2 (the hunting trials repaired, the reach and bell sweeps widened). The TSVs are the probe's stdout; the figures below are read from them.

## 1. A projectile landing near a roaming warden

- **A sourceless snowball landing within hearing turns the warden to the spot, and it walks there.** [cited, T1] Landing 7.5, 14.5 and 14.6 blocks from the warden's feet: the warden reached the landing cell (within 0.9 blocks) in 8–9 s each time, at about 1.9 blocks per second, stood on it for about 3 s, then roamed off. Landing 23.1 blocks away: it did not come within 15 blocks of the cell in 15 s.
- **Hearing reaches about 15 blocks from the warden's feet.** [cited, T1 run 2 — see the sweep at 8, 12, 16, 18, 20 and 24 blocks] The wiki's *"15 block radius ovoid"* is the published figure; the sweep states where this server's warden last turned and first did not.
- **Hold is short.** [cited, T1] About 3 s standing at the spot after a walk of 8–9 s, then roaming again: one snowball buys the walker roughly twelve seconds of the warden's attention, most of it spent walking to the sound.

## 2. A warden already hunting

- **An invulnerable body is not a target.** [cited, T2/T3 run 1] Anger seeded at 150 against a `NoAI, Invulnerable` zombie read back as `[]` on the next tick and the warden never moved toward it. The repaired trials use a body under Resistance V, which the warden targets and cannot kill.
- **Whether a projectile pulls a hunting warden off its target** is the run-2 measurement of T2 — a sourceless snowball first, then eggs thrown by the bot from a platform 10 blocks east and 11 blocks up — and the reading is three columns: the warden's distance to its target, its `attack_target` memory, and its `anger.suspects`.
- **What the thrower earns** is the same trial's `anger.suspects` row for the bot's UUID, and the bot's health: the published sonic boom reaches *"a 14-block radius horizontally and 20 blocks vertically"*, which the platform in the trial is inside.

## 3. Pursuit down a corridor

Run 2, T3: the warden starts at one end of an open-top lane with walls four high, its target 17 blocks down it; the reading is the time to come within 2 blocks of the target, for a lane 1 wide and for a lane 2 wide. The engine's own model gives the warden a 1 × 3 footprint (`crates/delvec/src/compiler/nav/world/body.rs`, `"warden" => (0.9, 2.9)`), so a 1-wide, 3-high lane is routable on paper; the trial says whether vanilla's pathfinder agrees.

## 4. Sneaking

- **A player walking 10 blocks from a calm warden is dead in about thirteen seconds.** [cited, T4-walk-at-10, easy difficulty] Anger at the bot read 34 at 4.4 s, 66 at 7.0 s, 119 at 9.5 s, 149 at 11.9 s; the bot died before the 14.6 s reading. The warden closed from 10 blocks to melee during the climb.
- **A player sneaking 10 blocks from a calm warden, walking to and fro for 15 s, is not heard.** [cited, T4-sneak-at-10] The bot's anger entry never appeared while the warden roamed between 8 and 12 blocks of it. (A stranger's entry did appear and climbed to 118: a chicken hatched from an egg thrown in an earlier trial, walking in the open. Run 2 kills chickens between trials.)
- **Sneaking does not hide a player from the sniff.** [cited, T4-sneak-stand-at-4] A bot sneaking and standing still 4 blocks from the warden for 20 s held an anger entry of 52 at the end: the sniff found it (the wiki's *"35 anger to the nearest mob or player within a 6 block radius horizontally"*), and it was below the 80 that opens a hunt only because the trial ended.
- [authored] Sneaking is the walker's tool against a warden more than 6 blocks away and useless inside that radius; the maze's lanes decide which case the walker is in, so a lane that passes within 6 blocks of the warden's roaming ground is a lane the walker cannot sneak.

## 5. A bell

Bell at 10 blocks east of a calm warden unless stated; the reading is the warden's feet.

- **A bell rung by redstone power is heard, and the warden walks to it.** [cited, T5-redstone-block-10] A redstone block set beside the bell: the warden reached the bell (1.6 blocks) in about 5.5 s and stood there. The bell's ring is a vibration.
- **A bell set directly to its `powered=true` state does not ring.** [cited, T5-state-powered] `setblock … bell[…,powered=true]`: the warden roamed to 5.2 blocks of it and no nearer, the same as the control.
- **`playsound` alone is not heard.** [cited, T5-playsound-only] `playsound minecraft:block.bell.use` at the bell's cell: the warden roamed to 5.5 blocks and no nearer. A sound is not a vibration.
- **How far a rung bell reaches** is run 2's sweep at 10, 15, 16, 18 and 20 blocks against a control with no ring.
- [authored] The engine's way to ring a distant bell is therefore a region write that powers it — a redstone block set in a cell beside it and cleared again a few ticks later — and never the bell's own state or a sound. Whether the DSL's `set-block` reaches that cell is the capability question `compiler.md`'s `set-block` row answers (it takes an anchor and a blockstate suffix); the cooldown and the limited uses are a `sequence` and a `state` datum, which the DSL already has.

## 6. The emerge animation

Run 2, T6: four summons at the same cell — bare, `{}`, the compiler's awake NBT, and the awake NBT plus `"minecraft:is_emerging":{value:{},ttl:133L}` — with the bot 12 blocks away reading the warden's synced pose (entity metadata index 6: 13 is EMERGING, 14 DIGGING, 0 STANDING on this version) every 250 ms, and the warden's brain memories and presence read at 12 s.

## 7. What the engine takes from it

- **The guide's steering has a reach of about 15 blocks from the warden's feet and buys about twelve seconds.** [authored, from §1 and §5] A platform must stand within that reach of the lanes it is to clear, and a bell's cell must be within it of where the warden is wanted, not of where it is.
- **Walking is detection; sneaking is detection inside 6 blocks.** [authored, from §4]
- **A bell is rung by power.** [authored, from §5]
