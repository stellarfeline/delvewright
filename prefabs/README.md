# prefabs/

The prefab generators' cargo workspace: GPL code that writes structure `.nbt`
files and their metadata. The `.nbt` library itself lives in the content
repository [`delvewright-campaigns`](https://github.com/stellarfeline/delvewright-campaigns)
under `prefabs/`; for local dev it resolves through the `campaigns/` symlink at
`campaigns/prefabs/`, and CI checks it out at the SHA `versions.toml` `[content]`
pins. The root workspace excludes this directory, so nothing here enters the
shipped `delvec`.

## Members

- **`invariants/`** — `prefab-invariants`, the rules every generator shares
  (block registry, derived connections, walk plane, waterline, preserving
  document writes). A library; its own unit tests prove each rule fails.
- **`generator/`** — the stone-keep tileset (`keep-*`). See `keep-tileset.md`
  and `generator/README.md`.
- **`cave-generator/`** — the cave/shore tileset (`cave-*`). See
  `cave-tileset.md` and `cave-generator/README.md`.
- **`island-generator/`** — the island set pieces (`island-beach-camp`,
  `island-galley`). See `island-tileset.md` and `island-generator/README.md`.
- **`island-terrain-generator/`** — the island terrain (`island-greenfield`,
  `island-greenfield-bend`, `island-mountain`). Same convention,
  `island-tileset.md`.
- **`hello-room-generator/`** — `hello-room`, the piece most engine test
  fixtures bind.
- **`rig-generator/`** — `rigs/tentacle/rig.json`, an assembly rig (spec-0082).
- **`../gallery/prefab-generator/`** — the gallery's own pieces, skins, design
  pictures, textures and heightmap, written into a build directory rather than
  the library. It lives beside the gallery, its only reader, and is a member of
  this workspace (`[package] workspace = "../../prefabs"`).

Every generator takes `<out_dir>` as its first argument and refuses one that
does not exist. The commands, the CI wiring and the shared invariants are
documented in `docs/reference/tools.md` §9.
