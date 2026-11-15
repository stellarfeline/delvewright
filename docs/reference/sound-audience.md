# Who hears a sound, and how a macro writes a number

What the pinned game does when a sound is played — by the `playsound` command and by a block's own refused use — and how a function macro writes a decimal into a command. Reader: the engine work that emits a sound for a party of several players (the refusal cue, `play-sound`, the quest cues) and the cutscene return's macro `tp` (`docs/reference/compiler.md`, §4 "A cutscene moves nobody"). Established once against the pinned jar and treated as constant; the read site is stated so the reading can be repeated when the pin moves.

## The read site

- The server jar `versions.toml [minecraft].server_jar_url`, sha256 `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726` (the pin), its inner `META-INF/versions/1.21.11/server-1.21.11.jar`.
- The ProGuard mappings piston-meta publishes for the version, `downloads.server_mappings`, sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`, which give every class and member below its Mojang name.
- `javap -p -c -constants` from a JDK 21 (21.0.13) over `PlaySoundCommand`, `SoundEvent`, `ServerLevel`, `BaseContainerBlockEntity`, `SoundSource` and `MacroFunction`.

## `playsound` (cited)

1. **`PlaySoundCommand.playSound(source, targets, sound, category, pos, volume, pitch, minVolume)`** computes the range once as `SoundEvent.getRange(volume)` squared, then walks `targets`: a target in another level is skipped; a target whose squared distance to `pos` is within the range is sent the sound **at `pos`**, at `volume`; a target beyond it is skipped when `minVolume` ≤ 0, else sent the sound at `minVolume` from two blocks along the line from the player toward `pos`.
2. **`SoundEvent.getRange(volume)`** is the event's fixed range when it has one, else `16 × volume` for a volume above 1 and `16` otherwise.
3. **The result** is the number of players the packet was sent to. None is the command's failure (`ERROR_TOO_FAR`); one or more is success with that count, so `execute store result score … run playsound …` stores how many players it reached.
4. So **the targets are a filter, never the audience's position**: `playsound <s> <c> @s` reaches the one executing player and no other, wherever the sound plays, and `playsound <s> <c> @a <pos>` reaches every player within the range of `pos`.

## A block's own refused use (cited)

1. **`BaseContainerBlockEntity.sendChestLockedNotifications(pos, player, name)`** writes `container.isLocked` to the **presser's** action bar (`Player.displayClientMessage(…, true)`), and, on the server, calls `Level.playSound(null, pos.x, pos.y, pos.z, SoundEvents.CHEST_LOCKED, SoundSource.BLOCKS, 1.0, 1.0)`. Its caller passes `BlockPos.getCenter()` of the container.
2. **`ServerLevel.playSeededSound(entity, x, y, z, sound, category, volume, pitch, seed)`** broadcasts the sound packet through `PlayerList.broadcast` to every player in the level within `SoundEvent.getRange(volume)` of the point, excepting `entity` only when it is a player; with `null` nobody is excepted.
3. So **the pinned game's refused use is heard by everyone near the block** — at the block's centre, in the `block` category, within 16 blocks — and **read by the presser alone**. The engine's refusal cue follows it (`press::refusal_cue_line`).

## A number in a macro (cited)

1. **`MacroFunction.stringify(tag)`** writes a `FloatTag` or a `DoubleTag` through a `DecimalFormat("#", DecimalFormatSymbols.getInstance(Locale.ROOT))` with `setMaximumFractionDigits(15)`, and a byte, short, int or long through `String.valueOf`. So a double stored as `45.5d` is substituted as `45.5`, with no type suffix and no grouping, and a macro `tp @s $(x) $(y) $(z) $(yaw) $(pitch)` reads it as coordinates and a rotation.
