# delvewright-render

The GPU render arms of [`delvec`](https://crates.io/crates/delvec), the delve
creator for **Minecraft Java Edition 1.21.11** adventure maps: a prefab is
meshed and rasterised headlessly through Nucleation and wgpu into a
deterministic multi-angle shot set, and a fixture of the newest blocks the game
has is rendered and scanned for the missing-texture placeholder, so a texture
pack that has fallen behind the game fails loudly rather than shipping magenta.

`delvec` mounts it as `delvec render`:

| Command | What it does |
|---|---|
| `piece <in.nbt> -o <dir>` | The planned shot set of one prefab, plus any camera you aim with `--view`. |
| `batch <dir> -o <out>` | The same for every prefab in a library directory. |
| `fidelity-gate` | Render the newest-block fixture and fail on any missing texture. |

Textures come from your own Minecraft client jar (`--textures <jar>` or
`DELVEWRIGHT_CLIENT_JAR`), which is never downloaded, bundled or
redistributed. Renders are review artifacts: on one machine, driver and wgpu a
double render is byte-identical, and across machines the committed test holds
them pixel-equal within a tolerance.

## Use

```toml
[dependencies]
delvewright-render = "1"
```

```rust
use delvewright_render::render::RenderParams;
use delvewright_render::shots;
```

## Compatibility

- **Minecraft**: Java Edition 1.21.11.
- **Campaign format**: `dsl_version` `0.2.0` through `0.19.0`.
- **Rust**: 1.97.1 or newer.

## Documentation

- [Tool reference](https://github.com/stellarfeline/delvewright/blob/main/docs/reference/tools.md)
  — the shot planner, the cameras you can aim, and the fidelity gate.
- [Project repository](https://github.com/stellarfeline/delvewright).

## Licence

GPL-3.0-only.
