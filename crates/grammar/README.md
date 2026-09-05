# delvewright-grammar

The box-split grammar prefab back end of [`delvec`](https://crates.io/crates/delvec),
the delve creator for **Minecraft Java Edition 1.21.11** adventure maps. A
grammar *program* — a typed JSON document of split, reorient and paint rules —
is expanded deterministically over an integer box and frozen as a vanilla
structure template plus its metadata, and every machine gate that judges the
result runs on the way out.

`delvec` mounts it as `delvec grammar`:

| Command | What it does |
|---|---|
| `list` | Every program in the rule library, with its knobs. |
| `show --program <id>` | A library program as the JSON it is authored in. |
| `check --file <ir.json>` | Validate a program's structure without expanding it. |
| `expand --file <ir.json> --region WxHxD -o <dir>` | Expand, judge and freeze a prefab. |
| `coverage` | Which constructs the library demonstrates, and which none of it does. |
| `audit --library` / `--campaign-root <dir>` | Expand and judge every program of a corpus. |

## Use

```toml
[dependencies]
delvewright-grammar = "1"
```

```rust
use delvewright_grammar::{Box3, ExpandOptions, expand, library};
```

## Compatibility

- **Minecraft**: Java Edition 1.21.11.
- **Campaign format**: `dsl_version` `0.2.0` through `0.19.0`.
- **Rust**: 1.97.1 or newer.

## Documentation

- [Grammar reference](https://github.com/stellarfeline/delvewright/blob/main/docs/reference/grammar.md)
  — the program form, the rule library and every gate.
- [Project repository](https://github.com/stellarfeline/delvewright).

## Licence

GPL-3.0-only. The rule interpreter is ported from a BSD-3-Clause work, whose
licence text ships beside this crate as `LICENSE-GDMC25`.
