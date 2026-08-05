# spec-0021 — Container loot + actor equipment

- **Status**: Draft (owner-directed 2026-08-03, from the drowned-bell playtest:
  the delve's chests and barrels were all empty, and the dormant elite stood
  there in no armour)
- **Scope**: two stage-5 surfaces, one shared enchantment vocabulary.

## Problem

Prefabs furnish rooms with chests and barrels, but nothing could put anything
*in* them — the only container the compiler ever filled was the one a `collect`
objective conjured with a hardcoded `setblock … minecraft:chest`. And `actors[]`
took no gear at all, so a set-piece elite could not be dressed, while a
throwaway wave mob could.

## Design

### `loot[]` — contents for furniture the prefab already placed

```json
{ "id": "loot/galley-stores", "anchor": "anchor/galley-crate",
  "items": [ { "item": "minecraft:cooked_cod", "count": 3 },
             { "item": "minecraft:paper", "name": "Tide Ledger" } ] }
```

The container is **prefab hardware**, exactly as a trap's dispenser is: the
compiler fills it and never places it. This is the no-hacks boundary — furniture
belongs in the piece, and a `loot` entry that cannot find a container is a
content defect, not something to paper over by setblock-ing a chest at runtime.

Slot assignment is **positional**: the nth declared stack goes to
`container.<n>`. That is the whole determinism story (ADR-0006) — no loot
tables, no weights, no seeded shuffle to reproduce.

Emission is the vanilla primitive the codebase already uses for trap dispensers
and `collect` chests: `item replace block … container.<n> with <item> <count>`,
with `custom_name` / `enchantments` as item components when declared.

### Actor `equipment`

`actors[]` gains `equipment` in **literally the wave-mob type**, so the two
surfaces cannot drift apart. It is emitted into both the puppet summon and the
unleashed twin's NBT: unleashing swaps the body, not the costume. Drop chances
are zero on every slot — an actor's kit is never farmable (no-grind).

It deliberately does *not* inherit the wave path's armed-mob default table: an
actor is a directed set piece that wears exactly what was declared, which also
keeps every campaign written before this field byte-identical.

### Amendment (2026-08-05, owner-approved, task #189) — `equipment.head` is enforced, not merely offered

`equipment.head` carries an owner ruling recorded on the DSL field itself: it is
**the** sanctioned answer to daylight-burning undead, and `set-time` never is.
Until now that ruling was advice — nothing checked it, and `hollow-vigil` shipped
a wave that burned to death before the party could fight it, with the whole
machine ladder green (owner playtest, 2026-08-05).

`DW0496` makes the ruling a compile-time obligation on **both** surfaces this
spec governs, since they share one type: a wave stack a `kill` objective
adjudicates, and an actor the party can damage (`vulnerable`, or unleashed).
The rule fires when the species is in vanilla's own `#minecraft:burn_in_daylight`
tag and is not fire-immune, the delve is pinned to a clear daytime hour for its
whole length, open sky stands on walkable ground within one aggro radius of where
the body is staged, and the head slot is empty. Prescription: `equipment.head`,
or roof the arena. **Never `set-time`** — the diagnostic says so in as many
words.

One exception is part of the rule: a phantom burns *through* a helmet
(minecraft.wiki/w/Phantom), so for that species the head slot is no exemption and
the prescription names roofing instead. The species list is Mojang's tag,
vendored, never a table the compiler wrote (`data/PROVENANCE.md`); the full
behavior record is `docs/reference/compiler.md`'s `DW0496` row.

### Enchantments

Each equipped piece and each loot stack is either a bare item id or
`{item, enchantments: {<id>: <level>}}`. The plain string form is preserved, so
existing campaigns re-serialise unchanged. Emitted as the 1.21
`minecraft:enchantments` component.

Ids validate against the 43-id 1.21.11 enchantment registry, extracted from the
same pinned misode/mcmeta summary the item registry comes from and inlined like
`EFFECT_IDS_1_21_11` (small, stable — no new data file, no new injection point).

Levels are checked against what the component can *store* (`1..=255`), not
against each enchantment's survival maximum. Exceeding that maximum from a
command is legal vanilla and is precisely how a set-piece elite is built, so the
compiler does not overrule it; `0` is rejected because the game silently drops
it.

## Acceptance criteria

1. A `loot` entry whose anchor cell is not `chest` / `trapped_chest` / `barrel`
   in the assembled world fails the build with `DW0431`, naming the loot id, the
   anchor, the cell and the block actually found.
2. A `loot` entry with more stacks than the container has slots fails
   (`DW0432` at validation tier, `DW0431` at build tier against the real
   container).
3. Two `loot` entries on one anchor is `DW0435`.
4. A `loot` item id outside the pinned registry is `DW0143`; an anchor no prefab
   provides is `DW0142`.
5. An unknown enchantment id is `DW0433`; a level outside `1..=255` is `DW0434`;
   a level above the survival maximum validates clean.
6. Emitted fills are positional (`container.0`, `container.1`, …) and every
   emitted line validates against the vendored 1.21.11 command tree.
7. An equipped actor's gear appears in **both** the `spawn_actor_<id>` puppet
   summon and the `unleash_<id>` twin summon, with `drop_chances` 0 per slot.
8. An actor with no `equipment` emits byte-identically to before this spec, and
   never picks up an armed-mob default.
9. A `loot` item `name` enters the l10n inventory as
   `loot.<id>.item.<i>.name`.
10. Both surfaces are reserved (`DW0141`) under a pre-0.6 quests version.
11. (Amendment, task #189) A daylight-burning species staged for a fight whose
    walkable ground reaches open sky within one aggro radius, in a delve pinned
    to a clear daytime hour, fails the build with `DW0496` naming the encounter,
    the species and the sunlit cell. Adding `equipment.head` clears it; so does
    roofing the arena. A husk (not in `#minecraft:burn_in_daylight`), a wither
    skeleton (in the tag, fire-immune), a night or rained-on delve, and a roofed
    arena each build clean. A phantom is `DW0496` **with** a helmet, and its
    message prescribes roofing rather than the head slot.
