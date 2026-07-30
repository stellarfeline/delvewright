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

| Vendored file | SHA-256 |
|---|---|
| `items-1.21.11.json`    | `3965d9d5aabc0a2e6270b9e15c4faed76b67b93663d3136fa6ca6ca6f9371e8c` |
| `commands-1.21.11.json` | `8e48958913bbd604bc6a084fa04f139c6012fbe6706391c79b265158221ff6ac` |

## Not committed

The Mojang server jar is **never** committed (Mojang EULA, ADR-0010). It was not
downloaded on this host (Java 21 unavailable). If a future refresh runs the
official generator, add the jar path to `.gitignore` before generating.
