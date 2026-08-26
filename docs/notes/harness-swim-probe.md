# Can the bot swim? — measured against the pinned pathfinder

Answer: **it swims across, and it cannot swim up.**

The bot tier's acceptance criterion for a route the tide opens ("a bot walks the
credited leg") is conditional on this, and the condition is sharper than
"can it swim".

## Instrument

`mineflayer-pathfinder@2.4.5` — the exact version `harness/package.json` pins,
installed fresh and read from disk rather than from memory. `harness/src/executor.ts`
constructs `new Movements(bot)`, so the defaults below are what the harness gets.

## What the default `Movements` carries

`lib/movements.js`: `liquids` holds **water and lava** (lines 55–57) and
`liquidCost = 1` (line 26). There is no `allowSwimming` switch, because
traversal through liquid is on by default and priced rather than forbidden.

## Per motion, from the move generators

| motion | generated? | why, in the source |
|---|---|---|
| swim horizontally | **yes** | `getMoveForward`: the placement branch is skipped when `blockC.liquid`; `+liquidCost` when the node itself is liquid |
| leave the water onto ground **level with the surface** | **yes** | same branch, taken when `blockD.physical` |
| fall into water | **yes** | `getMoveDropDown`: `blockLand.liquid && blockLand.safe` is a valid landing |
| **rise within water** | **NO** | `getMoveUp` line 1: `if (block1.liquid) return` — no upward neighbour is generated at all from a liquid node |
| **climb from water onto a ledge one block up** | **NO** | `getMoveJumpUp` needs `blockC.physical` or it places a block; placing is unavailable in adventure mode |

## What this bounds

A route the rising tide opens is walkable by the bot **only if its exit stands at
the water level, not above it**. A leg that asks a body to gain height *within*
the water is unverifiable at the bot tier as the harness stands — not because
swimming is unsupported, but because ascent is not a move the pathfinder emits
from a liquid node.

This coincides with the engine's own climb-out band, which already refuses a
ledge higher than a swimmer's reach. The two agree; that agreement is what makes
the constraint cheap to hold rather than a second rule to remember.

## Not established

Whether a live bot, placed in water by a rising level, is carried upward by
vanilla physics regardless of what the pathfinder plans — the pathfinder decides
where it TRIES to go, and buoyancy is not a plan. Settling that needs a running
server, and it changes nothing about which legs can be *proven*: a plan the
pathfinder cannot emit is a leg the tier cannot assert.
