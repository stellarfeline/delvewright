# Vendored Minecraft 1.21.11 data — provenance

These files pin the game data the compiler validates against (ADR-0009 = MC
1.21.11, ADR-0011 = vendored command tree + item registry). They change only if
ADR-0009's revisit triggers fire.

## Route taken

Mojang's official data generator was **not** run locally: it requires Java 21 and
only Java 17 was available on the build host (`java -version` →
`openjdk 17.0.20`). Per spec-0002 task guidance, the fallback route was used:
the 1.21.11 **summary** was fetched from the community-maintained
[`misode/mcmeta`](https://github.com/misode/mcmeta) repository, which republishes
Mojang's generated reports verbatim per version.

`misode/mcmeta` is a mirror of Mojang's own generated data (produced by the same
`--reports` generator), so the item registry and command tree here are Mojang's,
not third-party reconstructions.

## Sources

- Repo: `https://github.com/misode/mcmeta`
- Ref: tag `1.21.11-summary` @ commit `c976eb3b2cfcb9f205171527dec46b266afa3ac9`
- Retrieved: 2026-07-30
- `version.json` confirms: `id 1.21.11`, `data_version 4671`,
  `data_pack_version 94` (minor `1`) — i.e. pack format 94.1.

### Downloaded source files (checksums as fetched)

| Source URL (raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/…) | SHA-256 |
|---|---|
| `registries/data.min.json` | `7efb184902cfef62b431bc9826ebcbcde2c23746e5624326ffcf922e15cf28f9` |
| `commands/data.min.json`    | `f2477dfadbeff5707dce1083f90d5dc88f9130bf860ac1c134ffc1de1982b7f6` |
| `item_components/data.min.json` | `51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4` |
| `data/damage_type/data.min.json` | `0ce7edc377446ecddfd1c3b74b32e2dc3b248edc4035275134fb821e98a6c7ad` |
| `data/tag/damage_type/data.min.json` | `794ce6343293660b5f32d6a78f7a374623bb785d18dfc5ce3cbdeb3093b0161d` |
| `data/tag/entity_type/data.min.json` | `5523f45b7ddb178cd9f8bbe998458cc070910a74bc6c551a37b9279f5d73f844` |
| `version.json`              | `be02c05f3cce0e39a4ae855c01b3dda2f572078d575f4b6b2fd824cc8a137d62` |

## Vendored files (derived, committed here)

- **`items-1.21.11.json`** — the `item` registry array from `registries/data.min.json`,
  each id namespaced (`minecraft:<id>`) to match DSL usage, de-duplicated and sorted.
  1505 items. Deterministic transform: `sorted(set("minecraft:"+i for i in item))`,
  pretty-printed with sorted keys.
- **`commands-1.21.11.json`** — the Brigadier command tree from `commands/data.min.json`,
  re-serialized deterministically (`json.dumps(sort_keys=True, indent=2)`) so it is
  diffable and byte-stable. Semantically identical to the source (key order only).
- **`entities-1.21.11.json`** — the `entity_type` registry array from
  `registries/data.min.json`, each id namespaced (`minecraft:<id>`), de-duplicated
  and sorted (same transform as the item registry). 157 entity types. Validates
  v0.3 wave mobs (`DW0173`).
- **`sounds-1.21.11.json`** — the `sound_event` registry array from
  `registries/data.min.json`, each id namespaced (`minecraft:<id>`), de-duplicated
  and sorted (same transform as the item/entity registries). 1838 sound events.
  Validates v0.6 `play-sound` / v0.4 `narrate.sound` ids (`DW0326`, spec-0014).
  **Reproduce it** (not a one-off — CLAUDE.md debug doctrine "automate the pitfall
  out of existence"): `python3 tools/extract-sound-registry.py <registries/data.min.json>
  crates/compiler/data/sounds-1.21.11.json`. The script pins and checks the source
  SHA-256 and applies the transform `sorted(set("minecraft:"+i for i in sound_event))`,
  `json.dumps(indent=2, sort_keys=True)`.

- **`item-stack-sizes-1.21.11.json`** — every item's `minecraft:max_stack_size`
  default component, from `item_components/data.min.json` in the same summary,
  namespaced to match DSL usage. 1505 entries — exactly the key set of
  `items-1.21.11.json` (a test pins that, so a future regeneration cannot let the
  two drift). Validates that a single-slot fill's `count` fits the stack
  (`DW0436`): `item replace block … container.<n> with <item> <count>` fails
  **silently** above the cap (rabbit stew caps at 1), the same silent-failure class
  `DW0431` exists for.
  **Reproduce it**: `python3 tools/extract-item-stack-sizes.py
  <item_components/data.min.json> crates/compiler/data/item-stack-sizes-1.21.11.json`.
  The script pins and checks the source SHA-256, and refuses to default a missing
  component rather than silently assuming 64.

- **`item-combat-1.21.11.json`** — every item's `attack_damage` / `attack_speed` /
  `armor` / `armor_toughness` contribution, summed from the `add_value` modifiers of
  its `minecraft:attribute_modifiers` default component, plus its `minecraft:food`
  `nutrition` (the sustain term `DW0474` reads), from the same
  `item_components/data.min.json`. 127 entries (only items with a non-zero number).
  Feeds the spec-0023 winnability arithmetic (`DW0472`). **Absence is
  a fact, not a gap**: an item missing here has no combat *attribute*, which is not
  the same as dealing no damage — a bow's damage is projectile code and appears in no
  vanilla data at all, which is exactly why `combat.rs` treats a projectile kit as
  "TTK not provable" instead of "TTK infinite".
  **Reproduce it**: `python3 tools/extract-item-combat-stats.py
  <item_components/data.min.json> crates/compiler/data/item-combat-1.21.11.json`.
  The script refuses any non-`add_value` operation rather than mis-summing it.

- **`damage-types-1.21.11.json`** — every damage type's `scaling` field plus its
  membership of the vanilla `#minecraft:bypasses_armor` tag, from
  `data/damage_type/data.min.json` + `data/tag/damage_type/data.min.json`. 50 entries.
  Feeds the spec-0023 incoming-damage arithmetic (`DW0473`). **The finding this table
  pins**: eight of the nine damage types the DSL exposes are
  `when_caused_by_living_non_player`, and `damage-players` emits a bare
  `/damage <target> <amount> <type>` with **no attacker** — so an Easy campaign's
  scripted hits are *not* halved by the `min(dmg/2+1, dmg)` formula. Only
  `minecraft:explosion` (`always`) scales. Deriving the arithmetic from the
  difficulty formula alone would have been wrong by 2× in the lenient direction.
  **Reproduce it**: `python3 tools/extract-damage-types.py <damage_type/data.min.json>
  <tag/damage_type/data.min.json> crates/compiler/data/damage-types-1.21.11.json`.

- **`entity-tags-1.21.11.json`** — vanilla's built-in `entity_type` tags, from
  `data/tag/entity_type/data.min.json` in the same summary: tag id (namespaced,
  so a lookup reads like the `#minecraft:<tag>` a datapack would write) → its
  sorted values. 46 tags. These are **Mojang's own answers to "which entity types
  do X"**, which is the only acceptable source for such a question here — the
  alternative is a hand-written species table, i.e. exactly the invented vanilla
  data this file's next section refuses.
  Feeds `DW0496` (daylight-burning staging) via `#minecraft:burn_in_daylight`,
  the tag the 1.21 engine itself tests before running a mob's sun-burn tick. The
  tag is about which types run that tick, **not** about which types the fire then
  hurts: `minecraft:wither_skeleton` is in it and is fire-immune, and fire
  immunity is a hardcoded entity-type property that appears in no vanilla data
  branch — so `daylight.rs` carries that one exclusion explicitly, cited, rather
  than pretending the tag alone is the whole rule.
  **Reproduce it**: `python3 tools/extract-entity-tags.py
  <data/tag/entity_type/data.min.json> crates/compiler/data/entity-tags-1.21.11.json`.

### What vanilla data does NOT provide (and what the compiler does about it)

Mojang publishes no per-entity default attributes — mob base `max_health` and
`attack_damage` live in code, and no branch of the mcmeta summary carries them.
The winnability arithmetic therefore runs its numeric time-to-kill bound **only**
where the campaign declares `attributes.max_health` on the stack, and says so out
loud (`DW0475`) rather than inventing a health table. Inventing one is the
"invented precision" this codebase already refuses for `DEFAULT_FOLLOW_RANGE`
(`nav.rs`) and `MODEL_MARGIN` (`clearance.rs`).

## Default-font glyph metrics (measured, not vendored)

`crates/compiler/src/textfit.rs` carries the vanilla default font's glyph **advance
widths** (`DW0330`'s width model). These are *measured from the client jar*, which is
EULA-bound and must never be committed — so the numbers live as a Rust constant and
the measurement is reproducible instead of vendored.

**Reproduce it** (debug doctrine — "automate the pitfall out of existence"):

```sh
python3 tools/extract-font-metrics.py <minecraft-1.21.11-client.jar>
```

Stdlib-only (its own PNG decoder — the sheets are 1-bit indexed + `tRNS`). Prints a
JSON report; `ascii.advances` is the 95-entry table (index = codepoint − 0x20) that
must equal `ASCII_ADVANCE`, and `bottom_line` carries the full-width advance.

What it establishes, all verified against 1.21.11 client bytecode rather than assumed:

- **Provider order is first-wins.** `minecraft:default` stacks
  `space → nonlatin_european → accented → ascii → unihex`. (`FontManager` prepends
  each provider then reverses the list; the two inversions cancel.) Only `ascii.png`
  serves printable ASCII.
- **Bitmap advance** = `round(inkColumns * height / cellHeight) + 1`. The ASCII sheet
  is 8×8 at height 8, so advance = ink + 1. 68 of 95 printable ASCII are 6 px.
- **Unihex advance** = `(right - left + 1) / 2 + 1`, but `size_overrides` in the font
  definition pin the CJK blocks to columns 0–15 and win outright over measured ink —
  so every Han glyph and every full-width punctuation mark is **9**, against a Latin
  letter's 6. Ratio **9:6 = 1.5**, not the 2× a "CJK counts double" rule assumes.
- **The trap**: `— – … " " ' ' ·` sit next to the CJK blocks and are common in Chinese
  copy, but `nonlatin_european.png` is declared *before* `unihex`, so they resolve to
  bitmap glyphs (9, 7, 8, 5, 5, 3, 3, 2) — **not** full-width. `PUNCT_ADVANCE` pins them.
- The unihex definition and `unifont.zip` are **not in the jar** (the jar's copy is an
  empty stub); they come from the downloaded asset store, which the script locates via
  the launcher's version manifest / asset index.
- Caveat: with the client's **Force Unicode Font** option the stack becomes
  `[space, unihex]` and Latin collapses to ~4, making the ratio 9:4. The model budgets
  against the vanilla default.

| Vendored file | SHA-256 |
|---|---|
| `items-1.21.11.json`    | `3965d9d5aabc0a2e6270b9e15c4faed76b67b93663d3136fa6ca6ca6f9371e8c` |
| `commands-1.21.11.json` | `8e48958913bbd604bc6a084fa04f139c6012fbe6706391c79b265158221ff6ac` |
| `entities-1.21.11.json` | `a10cc5f3dc042dfb632e87131823846011586d19bd97814bf62b1fa6e66160d2` |
| `sounds-1.21.11.json`   | `841adcd38b83410bed32d57bab909829ce796c1ecd959f2891fcafbf427bc16c` |
| `item-stack-sizes-1.21.11.json` | `a896955918220c489ab2225db6772cd417a0273d94d8dd691029572566e1b5ee` |
| `item-combat-1.21.11.json` | `362288eae4c77d9c53d91547b5735c00d739cafc95e1ab2ef57cd1343b9d29ff` |
| `damage-types-1.21.11.json` | `c3daed77f2557dc7fd784d373e74c1d67b45157bb812c8e4dee761db4696b6fd` |

## Not committed

The Mojang server jar is **never** committed (Mojang EULA, ADR-0010). It was not
downloaded on this host (Java 21 unavailable). If a future refresh runs the
official generator, add the jar path to `.gitignore` before generating.

## Derived list vendored elsewhere

- **Enchantment ids (43, 1.21.11)** — the `enchantment` registry array from the
  same `registries/data.min.json` above, namespaced and sorted by the same
  transform. Because it is small and stable it is **inlined** as
  `delvewright_dsl::registry::ENCHANTMENT_IDS_1_21_11` rather than committed as
  a data file here, matching the precedent set by `EFFECT_IDS_1_21_11`. Used by
  `DW0433` to validate actor/wave `equipment` and stage-5 `loot` enchantments
  (spec-0021).
