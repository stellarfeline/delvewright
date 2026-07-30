# "Stone keep" prefab tileset

Twelve original jigsaw-compatible prefab pieces (the first library seed for
ADR-0004 multi-piece assembly). All pieces are enclosed, `lit` per the spec-0001
lighting contract, and share one connection convention so any exit can mate with
any entrance. Generated deterministically by `prefabs/generator` (ADR-0006).

## Connection convention — "keep-socket-v1"

Every doorway is a **socket**: a 3-wide × 3-tall opening centred on a wall at
floor level, with a single `minecraft:jigsaw` block embedded at the opening's
bottom-centre (the wall cell). The jigsaw block entity is uniform across the
whole library:

| field         | value            |
| ------------- | ---------------- |
| `name`        | `keep:socket`    |
| `target`      | `keep:socket`    |
| `pool`        | `keep:pool`      |
| `final_state` | `minecraft:air`  |
| `joint`       | `aligned`        |

- **Symmetric.** `name == target == keep:socket`, so every socket is both a
  receiver and an initiator: any two sockets connect. Which sockets expand (draw
  a new piece) vs. which are consumed as the incoming connection is decided by
  jigsaw generation order, not by the piece.
- **`joint = aligned`** keeps pieces upright (no roll); only the four cardinal
  yaw rotations are tried when mating, so floors stay level and lighting is
  preserved under rotation.
- **`final_state = air`** turns the jigsaw block into the 1-block threshold gap
  after generation, leaving a clean 3×3 passage where two doorways meet.
- **`pool = keep:pool`** is a *placeholder*. The compiler owns the real pool
  documents — member weights + roles — in **`prefabs/pools.json`**
  (`pool/stone-keep`), and **is the jigsaw**: it solves the layout from the
  campaign seed and emits `/place template` per piece, reading these socket names
  only as a connectivity vocabulary (ADR-0004 amendment; `crate::solver`,
  spec-0002). The prefab only promises the socket geometry; the compiler owns
  layout policy. (Because assembly is `/place template`, not `/place jigsaw`, the
  worldgen `template_pool` registry is not needed at all.)
- The socket **local position** and facing of every piece are recorded in each
  metadata JSON under `connectors[]` (`local_pos`, `facing`, `opening`, `joint`).

`keep:pool` (and any pool) must be present at **world-load time** — worldgen
template-pool registries are *not* refreshed by `/reload`; the shipped delve
bakes the datapack so pools load at first boot (verified, M2).

Assembly verified live on pinned 1.21.11: `/place jigsaw keep:pool keep:socket 5
<pos>` produced a connected multi-piece run (corridors + rooms + a gate room),
pieces flush and passable at every socket.

## Lighting

`lit` profile (spec-0001): floor light ≥ 7 at every walkable cell. Sources are
**embedded** — `minecraft:glowstone` set into the ceiling (a lattice ≤ 6 apart in
rooms, a single central source in corridors/alcove). `measured_min_light` in each
JSON is the minimum block light over all walkable (air) floor cells, probed live
on 1.21.11 with the piece's **doorways sealed** (sky-light = 0, pure block light —
the conservative per-piece value; a lit neighbour only adds light). Measurement
method: a `location_check` light predicate swept over the floor. All twelve clear
the `lit` bar with margin.

## Pieces (measured floor-light minimums)

| id                       | size (X×Y×Z) | sockets     | anchors                          | min light |
| ------------------------ | ------------ | ----------- | -------------------------------- | --------- |
| keep-spawn-hall          | 9×5×9        | S           | `spawn`, `anchor/exit`           | 10        |
| keep-corridor-straight   | 5×5×7        | N, S        | —                                | 9         |
| keep-corridor-corner     | 7×5×7        | N, E        | —                                | 8         |
| keep-corridor-tee        | 7×5×7        | N, E, W     | —                                | 8         |
| keep-room-small-a        | 7×5×7        | N           | `anchor/npc-stand`               | 8         |
| keep-room-small-b        | 7×5×9        | N, S        | `anchor/npc-stand`               | 9         |
| keep-room-small-c        | 9×5×7        | N, E        | `anchor/npc-stand`               | 9         |
| keep-gate-room           | 7×5×9        | N, S        | `anchor/gate` (iron bars), `anchor/keeper-stand` | 9 |
| keep-shrine              | 9×5×9        | N           | `anchor/objective`               | 10        |
| keep-boss-hall           | 11×5×13      | N           | `anchor/boss`, `anchor/objective`| 8         |
| keep-alcove              | 5×5×5        | N           | — (dead-end)                     | 10        |
| keep-cross               | 7×5×7        | N, S, E, W  | —                                | 8         |

Palette: `stone_bricks` shell/floor/ceiling, `chiseled_stone_bricks` accents,
`glowstone` lighting, `iron_bars` gate. Anchors (`spawn` mandatory where
relevant, npc stands, gate, objective/boss markers) are metadata only and
resolved by the compiler against bound prefabs (spec-0001 anchors contract).

## Provenance / license

Original Delvewright assets, **GPL-3.0-or-later** (pipeline-code license per
`prefabs/LICENSE-ASSETS.md`). No third-party material ingested. Recorded in each
metadata JSON.

## Regenerating (deterministic, ADR-0006)

```sh
cargo run --manifest-path prefabs/generator/Cargo.toml --release -- prefabs/
```

Byte-identical on every run (double-run hash-checked). The generator is a
standalone crate (its own `[workspace]`), **outside** `crates/`, so it is not
part of the compiler workspace and never enters the shipped binary.
