# prefabs/tidal-keep-generator — the "tidal keep" tileset generator

Deterministic generator for the six-piece **tidal-keep** tileset: the souls-mode
set (barrow shore → gatehouse → wall walk → courtyard/chapel hub → cistern
undercroft → bell tower). A sibling of `prefabs/island-terrain-generator` and
`prefabs/cave-generator`: its own `[workspace]`, outside `crates/`, so it never
enters the shipped `delvec` binary and the keep / cave / island `.nbt` output
stays byte-identical (ADR-0006). It reuses the Delvewright primitive family
(splitmix64 PRNG, trilinear value-noise palette field, vanilla-structure `.nbt`
emit, keep-socket geometry, the gravity-substrate invariant, a derived static
block-light estimate); no third-party material ingested.

```sh
# from repo root — writes tk-{barrow-field,gatehouse,wall-walk,courtyard-chapel,
# cistern,bell-tower}.{nbt,json} into the content repo's prefab library
cargo run --manifest-path prefabs/tidal-keep-generator/Cargo.toml --release -- \
  <content-repo>/prefabs/
```

It also prints the `pool/tidal-keep` block to merge into that library's
`pools.json`. The pool is *printed*, never written: every `*.json` in the prefab
directory is parsed as prefab metadata, so a stray snippet file is `DW0346`.

Byte-identical on every run (double-run hash-checked). The convention, socket
datums, per-piece design intent and the full anchor inventory live in
`../tidal-keep-tileset.md`.

## Layout

| file | contents |
| ---- | -------- |
| `src/common.rs` | noise, palettes, the cell grid, `.nbt` emission, `tk:socket` geometry, metadata JSON, the light estimate + room-lighting helpers, the redstone-dust wirer, and the shared invariants |
| `src/barrow.rs` | `tk-barrow-field` — shore, barrows, the optional-elite ground |
| `src/gatehouse.rs` | `tk-gatehouse` — timed portcullis, boulder stair, mural flank |
| `src/wallwalk.rs` | `tk-wall-walk` — curtain wall, crenellations, ambush turret |
| `src/courtyard.rs` | `tk-courtyard-chapel` — muster yard, two breach lanes, chapel |
| `src/cistern.rs` | `tk-cistern` — undercroft, flooded bays, the shortcut loop |
| `src/belltower.rs` | `tk-bell-tower` — rope room, loft rafters, boss ring, drop |
| `src/main.rs` | piece specs, emission, anchor hygiene, the pool snippet |

## Invariants (why generation fails instead of QA)

Every debugging lesson from this tileset is pinned as an assertion, not prose —
`assert_route_walkable` (nav-model walkability of every promised route),
`assert_field_open` (the optional elite's bypass lanes), `sightline_clear` (the
loft perches are visible from the doorway), `assert_anchors_sane` (standable
footings, clear-and-dry volley slots, legal ids, real dispenser sockets,
in-bounds regions), `assert_stair_flanks_sealed` (no flight is enterable over its
side rail — the `DW0430` lesson), `wire_dust` (redstone supports and up-step
clearances) and `assert_no_unsupported_gravity`.
See the tileset doc for the full list and what each one caught.

CI runs this generator (and its four siblings) twice per PR, so a broken
invariant or a non-deterministic byte fails the build rather than the playtest.

## Debugging

```sh
TK_DEBUG_LIGHT=1 cargo run … -- /tmp/out     # per-region measured light + darkest cell
TK_PROBE=2,12,8,18 cargo run … -- /tmp/out   # labelled block dump around <salt>,<x>,<y>,<z>
TK_DEBUG_STAIRS=1 cargo run … -- /tmp/out    # every stair flank the seal pass closed
```
