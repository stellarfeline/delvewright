# delvewright-admit

Prefab admission for [`delvec`](https://crates.io/crates/delvec), the delve
creator for **Minecraft Java Edition 1.21.11** adventure maps. A converted or
generated structure template becomes a library-grade prefab here: its palette
is audited against an allowlist and a code-injection forbid, its jigsaw sockets
and named anchors are carved and declared, its lighting is measured with the
compiler's own light model, its catalog card is validated, and a browse world
lets a person walk the candidates and leave notes that come back as curation.

`delvec` mounts it as `delvec prefab`:

| Command | What it does |
|---|---|
| `audit <piece.nbt>` | Palette allowlist, code-injection forbid, and the piece's own spatial contract against its bytes. |
| `resolve-jigsaw <piece.nbt>` | Bake foreign worldgen jigsaw markers to their final block. |
| `socket <piece.nbt> --pos … --facing …` | Carve a jigsaw socket and declare it. |
| `anchor <piece.nbt> --name …` | Declare a named point or gate anchor. |
| `lighting <piece.nbt> [--write]` | Measure the light a body walking in would find. |
| `catalog validate <card.json>…` | Validate catalog cards, licence included. |
| `gallery <dir> -o <out>` | A browse world of candidate pieces. |
| `curate`, `curate-merge` | Harvest the notes left in it into the catalog. |

Every write goes through one definition of the prefab metadata document, so a
step that edits one part of it leaves every other part exactly as it found it.

## Use

```toml
[dependencies]
delvewright-admit = "1"
```

```rust
use delvewright_admit::audit::audit;
```

## Compatibility

- **Minecraft**: Java Edition 1.21.11.
- **Campaign format**: `dsl_version` `0.2.0` through `0.19.0`.
- **Rust**: 1.97.1 or newer.

## Documentation

- [Tool reference](https://github.com/stellarfeline/delvewright/blob/main/docs/reference/tools.md)
  — every flag, what each step measures, and what it refuses.
- [Project repository](https://github.com/stellarfeline/delvewright).

## Licence

GPL-3.0-only.
