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
