# delve-schem

Converts **Sponge schematics** (`.schem`, versions 2 and 3) into **vanilla
Minecraft structure templates** (`.nbt`) for the Delvewright prefab admission
pipeline (spec-0007, M3). It also runs the community-contract **safety strip**
(the NBT code-injection audit hook) and splits oversize schematics past the
vanilla 48-cube structure cap.

Output targets **Minecraft Java 1.21.11** (`DataVersion` 4671, ADR-0009) and is
**byte-deterministic** (ADR-0006): stable palette ordering, fixed block
iteration, `BTreeMap`-backed NBT, gzip mtime pinned to 0. Same input + args →
byte-identical output.

## Usage

```
delve-schem [--json] convert <input.schem> --out <output.nbt> [--split <max>] [--palette-report]
```

- `--out <path>` — structure `.nbt` to write. When the schematic is oversize the
  parts and a `<base>.split.json` manifest are written **beside** it and `--out`
  itself is not written (see Splitting).
- `--split <max>` — per-axis part cap (default **48**, the vanilla limit).
- `--palette-report` — print the full input block-state palette (sorted, one per
  line; a JSON array under `--json`) to **stdout**. This feeds the prefab
  admission block-palette allowlist audit.
- `--json` — render diagnostics as one JSON object per line.

**Streams**: diagnostics go to **stderr**; `--palette-report` goes to **stdout**
(otherwise unused, so it stays machine-parseable).

**Exit codes**: `0` ok · `2` input error (unreadable/unparseable schematic or bad
usage) · `3` output error (cannot write) · `≥10` internal error.

## Format coverage

| Feature | Sponge v2 | Sponge v3 | Notes |
| --- | --- | --- | --- |
| Root layout | flat root compound | root wraps `Schematic` | both auto-detected |
| `Version` dispatch | ✅ (2) | ✅ (3) | other versions rejected |
| Dimensions `Width`/`Height`/`Length` | ✅ | ✅ | read as unsigned shorts |
| `Offset` / origin | ✅ | ✅ | recorded in the split manifest; dropped for single output (vanilla templates carry no offset) |
| Palette (block states + properties) | ✅ `Palette` | ✅ `Blocks.Palette` | `name[k=v,...]` parsed into name + property map |
| Block data (varint array) | ✅ `BlockData` | ✅ `Blocks.Data` | LEB128, Sponge `(y*L+z)*W+x` order |
| Block entities | ✅ `BlockEntities` (inline data) | ✅ `Blocks.BlockEntities` (`Data` nested) | position-rebased; `Id`→lowercase `id`; forbidden ones stripped |
| Biomes | ignored | ignored | out of scope for v1 |
| Entities (mobs/items) | ignored | ignored | out of scope for v1 |

The output is a standard structure template: `DataVersion`, `size`, `palette`
(`Name` + optional `Properties`), `blocks` (`pos`, `state`, optional `nbt`), and
an empty `entities` list. Every palette entry states every property the block
has, so nothing reading the file needs a table of 1.21.11 default states to know
what a cell is.

## Safety strip (community-contract audit hook)

Structure-embedded command blocks are a code-injection vector, so the following
are **unconditionally removed** (replaced with `minecraft:air`, block entity
dropped), each producing a `DW0700` warning naming what was removed and where:

- **Command blocks** — `command_block`, `chain_command_block`,
  `repeating_command_block`.
- **Structure / jigsaw blocks** — `structure_block`, `jigsaw`.
- **Spawner family** — `spawner`, `trial_spawner`, `vault` (by block name), and
  block entities `mob_spawner`, `trial_spawner`, `vault` (by id).
- **Any block entity carrying an embedded command or spawner definition** — its
  NBT is scanned recursively for a `Command` key or `SpawnData` /
  `SpawnPotentials` / `spawn_data` / `spawn_potentials` keys.

Surviving block entities (chests, signs, banners, …) are carried through with
their payload intact, rebased to structure form (schematic-only `Id`/`Pos`/`x`/
`y`/`z` keys removed, lowercase `id` set).

Use `--palette-report` to review the full palette an audit will admit.

## Splitting

Vanilla structure templates cap each axis at 48. When any dimension exceeds
`--split` (default 48) the schematic is tiled into a grid of parts:

- Parts are named `<base>.x<i>y<j>z<k>.nbt` (`<base>` = the `--out` file stem),
  emitted in `x → y → z` grid order.
- A `<base>.split.json` manifest records how to reassemble losslessly.

Manifest schema:

```json
{
  "base": "castle",
  "data_version": 4671,
  "source_size": [60, 10, 60],
  "source_offset": [0, 0, 0],
  "part_max": 48,
  "grid": [2, 1, 2],
  "parts": [
    { "file": "castle.x0y0z0.nbt", "grid_index": [0, 0, 0], "offset": [0, 0, 0],  "size": [48, 10, 48] },
    { "file": "castle.x0y0z1.nbt", "grid_index": [0, 0, 1], "offset": [0, 0, 48], "size": [48, 10, 12] },
    { "file": "castle.x1y0z0.nbt", "grid_index": [1, 0, 0], "offset": [48, 0, 0], "size": [12, 10, 48] },
    { "file": "castle.x1y0z1.nbt", "grid_index": [1, 0, 1], "offset": [48, 0, 48],"size": [12, 10, 12] }
  ]
}
```

Each part's `offset` is the source-local origin of that part; placing every part
at its `offset` within a `source_size` volume reconstructs the original exactly.

## Limitations

- **Biomes and entities are ignored** (v1). Only blocks and block entities are
  converted.
- **`Offset` is dropped for single (non-split) output** — vanilla structure
  templates have no offset field. It is preserved in the split manifest.
- **No block-state migration.** Output is always tagged `DataVersion` 4671
  (1.21.11); a source with a different `DataVersion` produces a `DW0702` warning
  and its block-state strings are read as 1.21.11 states. Convert legacy
  schematics with the vanilla client first if the states differ.
- **Palette must be dense** (indices `0..N` with no gaps) and non-negative;
  malformed palettes are rejected as an input error.
- The strip's recursive `Command`-key scan is conservative: a surviving block
  entity that legitimately nests a `Command` key anywhere in its NBT is stripped.

## Development

```
cargo test  -p delvewright-schem     # round-trip, strip, determinism, split, CLI
cargo clippy -p delvewright-schem --all-targets -- -D warnings
```

Reference schematics are built in code (`src/fixtures.rs`) — no assets are
fetched from the network. Tests cover block-for-block round-trips (v2 and v3),
v2/v3 convergence, command-block strip + chest survival, double-convert
byte-identity, and split + lossless reassembly.
