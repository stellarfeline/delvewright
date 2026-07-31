# render-spike (spike-render-fidelity)

De-risks the spec-0007 / spec-0003 render layer. **Evidence, not shippable code.**
Not a workspace member (own `[workspace]`), so `cargo test --workspace` stays green.

Two tools evaluated for Minecraft **1.21.11** fidelity:

- **Nucleation** (MIT) — headless per-prefab renders of vanilla structure `.nbt`.
- **Chunky** (GPLv3, path tracer) — whole-world beauty shots.

Evidence PNGs: `../../spike-output/`.

## Nucleation — per-prefab

Nucleation has **no importer for binary gzip vanilla structure `.nbt`** (its
format manager only detects Sponge `.schem`, litematic, Bedrock `.mcstructure`,
MCEdit, world regions, and *text* structure SNBT). So `nuke-render` gunzips the
`.nbt` itself, reads the vanilla `size`/`palette`/`blocks` schema, and rebuilds
it as a `UniversalSchematic` via the public `set_block` API. Textures come from a
resource pack — the **1.21.11 client jar** — which is what determines fidelity.
Render is wgpu → Metal, off-screen, no display.

```sh
# prefabs (byte-identical to campaigns/prefabs/*.nbt):
cargo run --manifest-path ../../prefabs/generator/Cargo.toml -- /tmp/prefab-out
# 1.21.11 client jar (textures/models):  ~/.chunky/resources/minecraft.jar also works
#   url resolved from piston-meta; sha1 ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd
cargo build --release
./target/release/nuke-render /tmp/prefab-out/keep-spawn-hall.nbt \
    /path/to/client-1.21.11.jar ../../spike-output/keep-spawn-hall_a.png --yaw=45 --pitch=30

# 1.21.x newest-blocks fidelity probe:
./target/release/make_test_nbt /tmp/prefab-out/fidelity-test.nbt
./target/release/nuke-render /tmp/prefab-out/fidelity-test.nbt \
    /path/to/client-1.21.11.jar ../../spike-output/fidelity-test_a.png --yaw=25 --pitch=35
```

## Chunky — whole-world

The 1.21-capable core is a **snapshot only** (stable stops at 1.20.4). Fetch it
from the launcher's update site, download the 1.21.11 base assets once, then
render a hand-authored scene JSON. Delve worlds are **void/flat**: Chunky loads
the sparse structure fine, but there is no terrain to auto-frame the camera on,
so the camera must be aimed by hand (yaw≈π/2 faces −Z, **positive pitch = down**).

```sh
# snapshot core + libs (versions from https://chunkyupdate.lemaik.de/snapshot.json)
#   this spike used chunky-core-2.5.0-SNAPSHOT.474.g156e2bb  (built 2026-07-26)
mkdir -p ~/.chunky/resources
java -cp 'chunky-lib/*' se.llbit.chunky.main.Chunky -download-mc 1.21.11
# build the delve world, boot once so the datapack place-s structures, copy world out
delvec build crates/dsl/fixtures/valid/keep-trial -o validation/delve-output
EULA=TRUE docker compose -f validation/compose.yaml --profile play up --build server  # then docker cp :/data/world
java -cp 'chunky-lib/*' se.llbit.chunky.main.Chunky -scene-dir scenes \
    -render keep -reload-chunks -f -target 500 -threads 8
```
