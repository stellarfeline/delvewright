# Roadmap

What Delvewright does today, and what comes next.

## What you can do today

- **Make an adventure map from a sentence.** Describe the adventure in Claude Code, and Delvewright writes a story-driven Minecraft map for one to four friends: classes with their own gear, branching dialogue, and more than one ending.
- **Know it can be finished before you play it.** Every map is checked by machine before it reaches you, and a bot plays every story branch through to its ending.
- **Host it with one command.** A finished map is a single `docker run`; friends join from an unmodded Minecraft Java 1.21.11 client.
- **Play in your own language.** Players see the story's text in their client's language wherever the map ships that translation.
- **Play the maps already made.** [Nobody's Isle, Doune Castle: A Guided Tour, and Vesperhold](https://github.com/stellarfeline/delvewright-campaigns/releases).

## What comes next

1. **Polish from Vesperhold.** Small things players and creators noticed: a dialogue button shows its full line when you hover over it, and a fallen player waits a moment before respawning. Vesperhold 1.1 ships with them.
2. **Quests beyond a single line.** Quests that run side by side and branch, instead of one checkpoint after another. Quest points become real blocks you use, such as a lever, a button or a campfire, instead of a glowing marker that shows through walls.
3. **A map made from nothing but the prompt.** A new map is made from an empty folder using only `/new-delve`, with no hand edits. Anything that gets in the way is fixed, and the run starts over until it goes through cleanly.
4. **Bigger, denser castles.** Castles full of shortcuts that loop back on themselves and tightly packed rooms, once three playtests in a row turn up no mechanical problems.

## Further out

Ideas we intend to keep possible, with no date yet:

- **Modpack adventures.** The same approach at a larger scale: a curated modpack and a designed open world, with story set in natural terrain.
- **A survival hub.** A survival world you can step out of into a delve and back again.
- **Community maps.** Other people's adventures, published alongside ours.
- **Bedrock players.** Players on Bedrock Edition joining the same servers.
- **Other AI assistants.** Making maps with an AI assistant other than Claude Code.
