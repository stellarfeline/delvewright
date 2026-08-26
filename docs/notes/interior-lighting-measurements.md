# What I established myself, and with what instrument

Instrument for every engine number below: `delvec`, built **release** from engine worktree at
revision `86944766f9b77cc4ae365ab3c86830077051ddef`, forced compile, `cargo build` exit status
asserted `0` on its own line, binary written 2026-08-25 04:26. `delvec --version` prints
`delvec 1.1.0, dsl 0.17.0, mc 1.21.11` — the version string is NOT the instrument name; the
revision is.

Content instrument: `delvewright-campaigns` worktree, branch `feat/the-tower-is-lit-by-design`
cut from `origin/campaign/bell-r2` at `1cd1e4a91e2643e4f139ee723fed783ed6815e19`, clean.

## 1. The refusal reproduces exactly

    delvec build campaigns/the-drowned-bell-r2 -o <scratch>/out-baseline --prefabs <content>/prefabs
    BUILD EXIT=2

Diagnostic census of that run, taken from the run's own output:

| code | level | count |
|---|---|---:|
| DW0210 | error | 1 |
| DW0351 | warning | 1 |
| DW0467 | warning | 2 |

**Exactly one error**, `DW0210`, `area/bell-tower`, cell `[1804, 69, 95]`, light 0, threshold 3,
effective sky 4. Established.

## 2. `DW0210` reports one cell of one area per build — established in code, not inferred

- `crates/compiler/src/light.rs:995` `measure_undeclared` returns `Option<LightDiag>` — at most
  **one** diagnostic per area, the single darkest reachable cell.
- `crates/compiler/src/light.rs:906` sorts diagnostics by `(a.code, &a.message)`.
- `crates/compiler/src/emit.rs:488` `if let Some(diag) = relight.diagnostics.first()` — the build
  returns `Err` on the **first** diagnostic only.

Since every `DW0210` message begins ``area `area/<name>` has a reachable walkable cell at``,
sorting by message is alphabetical by area id. So the build names the alphabetically-first dark
area and one cell of it. This confirms the earlier report's claim **at my revision** (it measured at
`800c958e`).

## 3. The sky arithmetic — the durable fact, and it survives any campaign reset

`crates/compiler/src/light.rs:588` `effective_sky`: `Dusk | Night | Midnight | Dawn => 4`, and
`if base <= 4 { return base }` — **weather is irrelevant at night**. `world.json` declares
`"time": "night"`, `"weather": "clear"`, so `darkest_effective_sky` = **4**. The build's own
message says `effective 4`, independently agreeing.

`DARK_THRESHOLD` = **3** (`light.rs:45`). The test is `l < DARK_THRESHOLD`, so light 3 **passes**.

`flood` (`light.rs:500`) is a BFS decrementing **1 per step** through passable cells; sky seeds
any passable cell whose column above is open, at `effective_sky`. Propagation stops at `l <= 1`.

Therefore, in a night campaign:

| cell | light | verdict |
|---|---:|---|
| open to the sky | 4 | passes |
| one step from open sky | 3 | passes (3 is not < 3) |
| **two steps from open sky** | **2** | **fails** |

**Sky light reaches exactly two cells deep at night. Everything deeper needs a placed emitter.**
That is why an open arcade at the top of a tower still measures dark, and it is a fact about the
engine and the pinned game, not about any campaign's programs.

## 4. The gate is NOT a coverage problem — the arithmetic that refutes paving

Light falls 1 per step, so an emitter of level `E` holds a cell at 3 out to `E - 3` steps of
shortest passable path:

| emitter | emission | radius to light 3 |
|---|---:|---:|
| lantern / glowstone / sea lantern / shroomlight / froglight / campfire | 15 | 12 |
| torch / wall torch / end rod | 14 | 11 |
| soul lantern / soul torch / soul campfire / crying obsidian | 10 | 7 |
| glow lichen / enchanting table / ender chest | 7 | 4 |
| amethyst cluster | 5 | 2 |
| magma block | 3 | 0 (lights only itself) |
| candle (1–4) | 3/6/9/12 | 0/3/6/9 |

One lantern holds a 25-block-diameter sphere above the threshold. `DW0210` is therefore cleared
by **sparse, motivated fixtures**; the instinct to pave a floor comes from misreading a
minimum-brightness gate as a coverage requirement. Established by arithmetic over the flood rule
and the emission table, both read at my revision.

## 5. Candles emit what the game says they emit

This section recorded a modelling gap: `emission()` had no arm for candles or copper bulbs, both
fell through `_ => 0`, and a room lit only by candles was modelled at 0 and refused. The gap was
real and it is closed. `emission()` is now measured against the pinned server jar —
`BlockState.getLightEmission()` over all 29,671 blockstates — and `emission_table.rs` asserts
`emission ≤ game` over every one of them, so the never-overestimate contract is proven rather
than documented.

The census that closed it corrects this note's own figure: **thirteen** was a count of *families*.
The under-modelled population was **62 block ids in 16 families**, and three families are ones
this note never named. There was also one **over**estimate in the opposite direction, which a unit
test asserted: bare `glow_lichen` is the faceless default state, which vanilla lights at 0 where
the table returned 7.

Three ids remain deliberately below the game — redstone lamp, trial spawner, vault — because the
world re-derives their state at load and the model takes the minimum over the states it can reach.
A copper bulb is not one of them: shipped lit with no redstone nearby, it stays lit.

## 6. Three different passability predicates in three crates

| crate | predicate | passable set |
|---|---|---|
| `crates/grammar/src/nav.rs:68` | `Voxels::passable` | `is_air()` **or** name ends `_skull` |
| `crates/admit/src/light.rs:337` | `is_passable` | air + torch/wall_torch/soul_torch/redstone_torch, water, vine, glow_lichen, rail, light, structure_void |
| `crates/compiler/src/assembled.rs` | `occupancy_of` | air + pressure plates/tripwire, thin decoration (carpets, snow layers), fence gates |

The grammar one is the strictest and it governs zone programs. **Re-derived at my revision**;
The earlier report's statement of it is correct.

## 7. The campaign's existing light idiom (measured, before the reset instruction)

Grep for lighting vocabulary across the eight zone programs, resolving what each key matched:

| program | lamp/lantern roles | note |
|---|---|---|
| z0-barrow-shore | 1 `minecraft:lantern[hanging=false]` | `shelf/lamp`, the lampman's shelf |
| z1-cliff-road | 1 `minecraft:lantern[hanging=true]` | `store/lamp`, the rope store |
| z2-gate-ward | 1 `minecraft:lantern[hanging=false]` | `gate/lamp`, with `lamp/gap`=4, `lamp/run`=3 and rules `lamp/bracket`, `lamp/niche`, `lamp/mouth`, `lamp/air`, `lamp/fitout` |
| z3, z5, z6 | **none** | no light-emitting role at all |
| z4-chapel-ward | none (`lamp-step` is an anchor name) | |
| z7-bell-tower | **none** | |

**Correction to the brief, by a sharper method.** The brief said z7 has no lighting vocabulary and
that is TRUE, but a plain grep for `light` returns 11 hits in z7. Resolving what the key actually
matched: all 11 are the word **`flight`** (stair flights). z4's 10 are `flight` too; z1's include
`lightning_rod`. A `grep -c light` over these programs measures the word "flight".

## 8. What I did NOT establish

- **I did not re-derive the six-area dark-cell table.** `DW0210` surfaces one cell of one area per
  build, so enumerating the distribution needs a harness linking the compiler's `LightModel`
  against the assembled world. That harness did not survive into the diff that reported it (its changed files
  are the campaign program, `GENERATION.md` and five prefab pointers). I judged building one not
  worth the round once the reset instruction arrived, because the counts are counts **of the eight
  zone programs the reset voids**. What survives the reset is §3's arithmetic, which explains the
  table's shape without depending on it: at night, anything more than one step from open sky is
  below the bar, so "most reachable cells of every enclosed area" is the expected reading.
- I did not verify the earlier report's per-room breakdown of the tower's 1335 cells, nor its 317-cell belfry
  figure, nor the claim that a lichen bed broke three contract gates with 1120 cells cut off. I
  re-derived the *mechanism* behind that last one (§6) but not the measurement.
