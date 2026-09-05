# delvewright-orchestrator

Playtest harvesting for [`delvec`](https://crates.io/crates/delvec), the delve
creator for **Minecraft Java Edition 1.21.11** adventure maps. A playtester in
the running map leaves notes with one in-game trigger; the server log carries
them stamped with where the player stood and which objective was live. This
crate pairs those stamps with the creator overlay's layout manifest and writes a
versioned `playtest-report.json` — and, when the session proposed camera shots,
a `rehearsal-report.json` beside it.

`delvec` mounts it as `delvec harvest <server.log> <layout.json> [-o report]`.

## Use

```toml
[dependencies]
delvewright-orchestrator = "1"
```

```rust
use delvewright_orchestrator::{Layout, harvest, report_json};
```

## Compatibility

- **Minecraft**: Java Edition 1.21.11.
- **Campaign format**: `dsl_version` `0.2.0` through `0.19.0`.
- **Rust**: 1.97.1 or newer.

## Documentation

- [Tool reference](https://github.com/stellarfeline/delvewright/blob/main/docs/reference/tools.md)
  — the flags and the report's shape.
- [Project repository](https://github.com/stellarfeline/delvewright).

## Licence

GPL-3.0-only.
