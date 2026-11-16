# Which score holders the sidebar draws

What the pinned game draws in the `sidebar` display slot, for the reader who changes `state[].display` or `boards[]` (`docs/reference/compiler.md`, *state[].display*). Established once against the pinned 1.21.11 client jar (sha1 `ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd`) read with Mojang's client mappings for that version (`client.txt`, sha1 `031a68bebf55d824f66d6573d8c752f0e1bf232a`), and treated as constant.

## What the client does

- `net.minecraft.client.gui.Gui.displayScoreboardSidebar` (obfuscated `giq`) builds its rows from the objective's `PlayerScoreEntry` list and drops every entry whose `isHidden()` is true (the filter lambda at source line 703).
- `net.minecraft.world.scores.PlayerScoreEntry.isHidden()` (obfuscated `fuk.a()`) is `owner.startsWith("#")`. A holder whose name starts with `#` is never drawn, whatever its score.
- `PlayerScoreEntry.ownerName()` (`fuk.b()`) is the entry's `display` component when one is set, and the holder's literal name otherwise. `scoreboard players display name <holder> <objective> <component>` sets that component for one holder in one objective.

## What the engine takes from it

- A `party` datum's value lives on `#party`, which the sidebar never draws. A standing `party` datum is drawn through the holder `dw.party` in the datum's own objective: `setup` copies the party value onto it and sets its display name to the datum's translated `name`; the datum's announcer copies it again on every change.
- `dw.party` contains a `.`, which no player name can (`[A-Za-z0-9_]`), so the line never shares a row with a player.
- The line lives in the datum's own objective, so a `hide-board` that puts the standing datum back in the slot puts its line back with it.
