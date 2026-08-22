# Capability ceiling of mod-free (vanilla datapack) gameplay

Reference note for ADR-0003: how high the mod-free ceiling reaches, against a
pinned MC 1.21.11 (ADR-0009). Delve designs and DSL features must stay inside the "available" column; the "out of reach" list is
the honest price of the vanilla-first decision.

## Available on 1.21.11 (datapack + resource pack + commands)

**Quest & progression machinery** — advancements (custom triggers, trees, rewards),
scoreboards, function scheduling, per-player state. This is the delve's spine; fully
expressible.

**Server-driven UI — the big modern win**: the **dialog system** (added 1.21.6) gives
real GUI screens from the server: multi-button choices, text inputs, command-firing
buttons. Class selection, NPC conversation trees, and quest logs can be actual UI, not
chat-line hacks. Plus titles/actionbar/bossbar, sidebar, custom fonts (icons, styled
text), and the locator bar/waypoints (1.21.6) for guidance.

**Custom items/gear** — item components (1.20.5+): names, lore, attributes,
enchantments, unbreakable, food/consumable behavior, tooltips, `custom_data`,
`item_model` + CustomModelData with a paired resource pack for unique visuals. Class
kits and quest items are first-class.

**"Abilities"** — composited from advancement triggers + interaction entities +
scoreboard events + effects/attributes/teleports. Cooldown-based class skills are
routine adventure-map fare.

**Projectile impact responses** — data-driven enchantment effects give a real,
event-driven hook the moment a projectile lands (`minecraft:hit_block`), from
which any effect is reachable, including a non-block-breaking explosion and a
lingering area effect. Two constraints measured on the pinned build and worth
knowing before designing with it: the behaviour binds to the **launcher** item,
never to the ammunition (though the ammunition's `custom_data` survives to the
impact and can be branched on), and vanilla's own `explode` damage collapses to
a flat 1.0 when the impact point falls inside the block that was hit. Details
and the full axis table: [`area-effect-arrow-spike.md`](area-effect-arrow-spike.md).

**NPCs & bosses** — entities with AI disabled/NoAI + interaction entity hitboxes;
dialog-based conversations; bossbar + phased boss fights scripted as functions
(spawn waves, arena changes, attack patterns via effects/projectiles/teleports).

**Presentation** — display entities (block/item/text, with interpolated transforms)
for decoration, floating text, and simple animation; particles, custom sounds and
music via resource pack; `/playsound`, fades via titles.

**World** — full datapack worldgen: custom dimensions, biomes, and **jigsaw
structures** (ADR-0004's foundation); gamerule/mobGriefing/daylight control; adventure
mode with `can_break`/`can_place_on` item scoping.

**Multiplayer** — teams, per-player and party-wide scoreboard logic, `/transfer`
(v2 hub topology).

## Out of reach without mods (the real ceiling)

- **No custom camera / cutscenes.** Java has no `/camera` (Bedrock does). Best
  approximations: forced spectator + `/spectate`, slow display-entity "sets", title
  cards. Cinematic storytelling is the single biggest sacrifice.
- **No new blocks with behavior.** Visual fakery via display entities over barrier
  blocks works but costs entity count and click-handling complexity.
- **No custom entity models/animations.** Monsters are re-dressed vanilla mobs
  (equipment, names, sizes via attributes, display-entity costumes) — laborious and
  never as good as a modeled mod entity.
- **No custom mob AI.** We can gate, buff, equip, and teleport vanilla AI, not write
  new goals. Scripted NPC *movement* (walk this path, act this scene) is particularly
  clumsy — expect stationary NPCs.
- **No client input beyond vanilla.** No keybinds; interaction is clicks, movement,
  chat/dialog buttons, item use.
- **Tick-granularity scripting.** Everything runs in 50ms steps under
  maxCommandChainLength budgets — fine for 1–4 players, but rules out twitch-precise
  mechanics.

## Verdict (why ADR-0003 stands)

The intent of the design is controllability, and it delivers: every mechanic is data
emitted by our compiler, hence statically checkable, deterministic (ADR-0006), and
bot-walkable (ADR-0005) — none of which survives arbitrary mod code. For the delve
format — curated 2–3h quest adventures for 1–4 players — the ceiling binds almost
entirely on **presentation** (cutscenes, bespoke creatures), not on quest structure,
classes, bosses, or UI, where 1.21.11's dialog system raises the bar well above the
classic command-map era. If a future delve concept dies specifically on the
presentation ceiling, that's ADR-0003's revisit trigger — a deliberate decision,
not a drift.
